# Mission logic (Episode 2)

| | |
|---|---|
| **Spec ID** | `sim/e2-mission-logic` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (phase changes, spawns, removals, success and outcome decide the result of a flight; the order of the per-tick rules matters where two rules write the same ship) · T2 for the ticker text itself ([hud](../render/hud.md) §4.7) |
| **Confidence** | Architecture, primitives, comm viewer table, start views and outcome tests: CONFIRMED (code). Flights 1–9: LIKELY (static only: every rule read from a gap-free listing of `2236:0ADB..1D29`, push order of every call read from the instructions). Flights 10–20 and `Msn_E2FlightEvents`: rule text CONFIRMED (all 107 calls and every store of `2236:240A..31DD` accounted for), gameplay consequences LIKELY. Captures C-0015 (flight 1) and C-0016 (flight 9) confirm the start state and the tick-1 engagements of flight 1 (§7 vector C1); no capture yet of a phase change |
| **Tasks** | M10-01 |
| **Unknowns** | U-242..U-248; U-200 (resolved, §4.7.2), U-204, U-205 (resolved: [e2-mission-ships](e2-mission-ships.md)) |

Read this spec as a **delta against the Episode 1 spec** [mission-logic](mission-logic.md): concepts, notation and primitives that are the same are referenced, not repeated. Addresses are `td2:<seg>:<off>` (image-relative, ADG §7.8); the `td2:` prefix is dropped where unambiguous. DGROUP is segment `36B6`; `DS:x` means an E2 DGROUP offset.

## 1. Summary

As in Episode 1, every flight's logic is code: `Msn_UpdateA` (`2236:0ADB`, flights 1–9) and `Msn_UpdateB` (`2236:240A`, flights 10–20) run once per tick right after the player ship is drawn, and the F6 comm viewer advances the phase when a pending transmission is read. A flight's rules are a flat list evaluated in a fixed order every tick: a **base test** (is the player at the flight's base, usually after the objectives?), **exit rules**, **engagement rules** and a **phase script** keyed on `g_MissionPhase` (1 at start, 100 = objectives done). Against Episode 1 the framework changed in five ways: (1) the E1 per-tick event routine (respawns and scripted events) is gone, so **no ship ever respawns**, and a new routine `Msn_E2FlightEvents` (`0E96:99B8`) handles only flights 17 and 18 (§5.10–§5.20); (2) `Msn_NearLive` measures the player at tile (viewX + 5, viewY + 3), the tile the guidance line and the AI use, instead of the view's top-left tile; (3) the base tests differ per flight (other base tiles, some with a sub-tile limit, two without a phase condition); (4) the phase scripts have no "hide every tick" branches: ships that join later are hidden by setup and given a class by the script; (5) a ship destroyed by a projectile keeps its class until the AI pass of the same tick, so mission rules see the kill one tick later. The end-of-flight code computes an outcome flag for flights 3, 8, 9, 10, 11, 16 and 17 from where certain ships ended up.

## 2. Definitions and units

All terms of [mission-logic](mission-logic.md) §2 apply with these E2 addresses:

| Term | Original (E2) | Notes |
|---|---|---|
| Slot *s* | 1..20 | |
| Class(*s*) | `g_SlotClass` DS:39A1 + *s*, u8 | FFh = not in play |
| **Live** / **dead** | class(*s*) ≠ FFh / = FFh | "dead" also covers never spawned, hidden, removed by an exit rule, removed by the AI pass (armour < 1, exit marker, map edge) |
| Tile(*s*) | (`g_SlotTileX` DS:3A55 + *s*, `g_SlotTileY` DS:3A69 + *s*), u8 | Kept after the ship is removed |
| Phase | `g_MissionPhase` DS:31E2, u8 | Set to 1 by `Msn_Setup` (`0E96:60F9`) for every flight 1–9 |
| Ticker idle | `g_TickerPos` DS:31E4 = 0 (u16) | A post sets it to 1; it returns to 0 after 78 page-0 ticks (155 or 156 ticks, [hud](../render/hud.md) §4.7) |
| Comm pending | `g_CommIncoming` DS:31C4, u8 | 1 = the F6 indicator pulses ([hud](../render/hud.md) §4.8). The comm viewer does **not** test it (§4.6) |
| Marker(*s*) | `g_SlotMarker` DS:387F + *s*, u8, *s* = 1..21 | 0 none, 32h target, 39h friend, 3Dh home; element 21 (DS:3894) = home point. Flights 1–9 set markers only in setup; no rule changes them |
| Succeeded | `g_MissionSucceeded` DS:31C6, u8 | Cleared at flight start (`0E96:A7CA`); recomputed by the base test of the flight every tick |
| Lost / Over / End ticks | `g_MissionLost` DS:11F0 / `g_MissionOver` DS:31DF / `g_EndTicks` DS:31E0 | Not written by flights 1–9 |
| Outcome / Died | `g_MissionOutcome` DS:11F3 / `g_PlayerDied` DS:11F1 | Teardown (§4.8) |
| View tile, view offset | `g_ViewTileX/Y` DS:395E/3960 (i16), `g_ViewSubX/Y` DS:3962/3964 (i16, 16..48) | "subY" below is `g_ViewSubY` |
| Rule loop counters | `g_MsnI` DS:31E6, `g_MsnJ` DS:31E8 (u16) | As E1 DS:32BE/32C0; nothing else reads them. Flight 2 also uses one local byte of the routine's frame (§5.2) |
| Comm request, F6 key | `g_CommRequest` DS:162B; F6 key byte DS:19C4 (`g_KeyDown` + 40h) | §4.6 |
| Ship record | `g_Ships` DS:435A + (*s* − 1) × 429h | `AI_SetTarget` writes +41Fh, +415h, +41Bh ([mission-ships](mission-ships.md) §3.1) |

**Player positions.** E2 uses two conventions (E1 used three):

| Used by | Player position |
|---|---|
| `Msn_NearLive` with slot 0 (every engagement rule and every "near the player" condition) | **(viewX + 5, viewY + 3)**, low bytes. Same tile as the guidance line, the radar and `AI_UpdateShips` |
| Base tests (§4.4) | the view tile (viewX, viewY), sometimes with subY |

The E1 respawn distance convention (viewX + 5, viewY + 2) has no E2 counterpart (no respawns).

## 3. Data

### 3.1 Primitives (unit `2236`)

| Name | Address | Pascal signature | Meaning |
|---|---|---|---|
| `Msn_ExitAt` | `2236:05DC` | `(Slot, X, Y: Byte)` | If tile(Slot) = (X, Y): class(Slot) := FFh. No liveness test, no kill credit. Identical to E1 |
| `Msn_NearLive` | `2236:0616` | `(A, B, Range: Byte): Boolean` | Position of *k* = tile(*k*) for *k* ≠ 0; for *k* = 0 it is (low byte of viewX + 5, low byte of viewY + 3), the sums taken in 16 bits. True when \|xA − xB\| < Range **and** \|yA − yB\| < Range (both positions zero-extended to 16 bits, signed difference, strict compare) and neither A nor B (when non-zero) has class FFh. The test is symmetric in A and B |
| `Msn_PostTicker` | `2236:030A` | `(X, Y: Integer; S: String; Colour: Byte)` | Identical to E1: draws S opaquely into the ticker strip (always X = 0, Y = 735) with the mission glyph routine `2236:0000`. No E2 mission message contains a digit, so the glyph routine's digit-advance quirk ([hud](../render/hud.md) §4.7) never shows |
| `AI_SetTarget` | `2F63:3232` | `(Slot, Target: Byte)` | Identical to E1: record(Slot) +41Fh := Target (0 = player), +415h := 1, +41Bh := 1 |

The E1 helpers `Msn_PlayerTouches` (`2404:0616`) and `Msn_Near` (`2404:07DB`) do not exist in E2.

### 3.2 Notation used in the flight sections

As [mission-logic](mission-logic.md) §3.2, with these definitions:

| Notation | Meaning |
|---|---|
| `post(id)` / `post(id, c = 42)` | `Msn_PostTicker(0, 735, text of id, colour)` then ticker := 1; colour 37 unless written `c = 42`. In flights 1–9 every post is followed by ticker := 1 |
| `exit s @ (x, y)` | `Msn_ExitAt(s, x, y)` |
| `engage a → b, r` | `if Msn_NearLive(a, b, r) then AI_SetTarget(a, b)`; b = `player` is slot 0 |
| `engage a → b1, b2, …, r` | several `engage` rules with the same attacker and range, in the order listed |
| `spawn s as c` | class(*s*) := *c* |
| `hide s` | class(*s*) := FFh |
| `M1[k]`, `M2[k]` | resource `e2.flight_messages.01` / `e2.flight_messages.02`, element *k* (0-based); §3.4 |
| `dead(s, …)` | every listed slot has class FFh; `dead(a..b)` lists a range |
| `ticker idle` | `g_TickerPos` = 0 |

Several E2 rules make the range test with the pair reversed (`Msn_NearLive(b, a, r)` followed by `AI_SetTarget(a, b)`). Because `Msn_NearLive` is symmetric this is the same rule and is written `engage a → b, r`.

### 3.3 Per-flight start state that the rules depend on

Set by the mission file `m<n>.td2` (slot classes, path step 1 = start tile), `Msn_Setup` (`0E96:60F9`) and `Sim_FlyMission` (start view). The setup spec (another M10 task) is authoritative for armour, aggression and class-table patches; this table lists only what the rules below read.

| Flight | Objective (ticker at start) | Colour | Player class | Start view (offsets 32, 16) | Markers at start | Live at start (class) | Hidden at start |
|---:|---|---:|---:|---|---|---|---|
| 1 | `e2.objectives[0]` | 37 | 2 | (102, 104) | 1, 6..13 = 32h | 1 (2); 3, 4 (4); 6..8 (8); 9..13 (7); 15, 16 (1) | (empty: 2, 5, 14, 17..20) |
| 2 | `e2.objectives[1]` | 37 | 2 | (102, 104) | 4..8, 12..18 = 32h | 1, 2 (2); 4..7 (3, setup; file 7); **8 (3, setup; empty in the file, tile (0, 0))**; 12..18 (7) | — |
| 3 | `e2.objectives[2]` | 37 | 2 | (102, 104) | 1 = 39h; 3..8 = 32h | 1 (0, the escorted truck); 3..5, 7, 8 (7); 6 (8) | — |
| 4 | `e2.objectives[3]` | 37 | 4 | (102, 104) | 1..3 = 32h | 1..3 (0); 8..11 (7); 14..16 (3) (setup rewrites the same classes) | — |
| 5 | `e2.objectives[4]` | 37 | 7 | (179, 33) | 1..3, 5, 6 = 32h | 1..3 (0); 5..9 (3); 11 (7) | — |
| 6 | `e2.objectives[5]` | 37 | 7 | (179, 33) | none | 2, 4..11 (8); 13 (7, setup; file 8); 15, 16 (3) | — |
| 7 | `e2.objectives[6]` | 37 | 3 | (120, 70) | none | 2..13 (7 or 8) | — |
| 8 | `e2.objectives[7]` | 42 | 3 (late draw) | (102, 100) | 1..11 = 39h; 15..20 = 32h | 2, 3 (0); 4 (4, setup; file 2); 15, 19 (8); 16..18 (7) | **5..11** (setup; file classes 0, 0, 4, 0, 0, 0, 4); all at tile (107, 106) |
| 9 | `e2.objectives[8]` | 37 | 4 | (87, 28) | 5 = 32h | 2, 3 (4); 5 (0, the target truck, start (1, 30)); 8..10, 12..14 (7); 11, 15 (8) | — |

Every flight: markers 1..20 are cleared first and the home marker (element 21) is 3Dh; phase := 1; the objective is drawn into the ticker strip (x 0, VGA row 735) and ticker := 1. Home points (guidance target when no marked slot is live, `UI_DrawGuidanceLine` `0E96:8587`): flights 1–4 and 7 (107, 104); 5 (189, 37); 6 (125, 68); 8 (1, viewY + 2), recomputed every tick; 9 (92, 28) ([episode-deltas](../../re/episode-deltas.md) §6; verified).

Path ends that the rules rely on (from the mission files; path step *k* of slot *s* is at file offset 80 + (*s* − 1) × 501 + *k* − 1 for X and 10,080 + … for Y, [structures](../../re/structures.md) §2):

| Flight | Slot(s) | Path ends at | Rule that matches it |
|---:|---|---|---|
| 3 | 1 | … (87, 134), (87, 135), (86, 135), (85, 135), (84, 135) | arrival rule tests (85..87, 135) |
| 4 | 1, 2, 3, 8, 11, 14, 15 / 10, 16 | (1, 122) / (1, 123) | exit rules; slot 9's path loops near (155, 58) and never reaches its exit tile |
| 5 | 1..3, 5..9 | (50, 1) | none: the AI exit marker removes them ([mission-ships](mission-ships.md) §4.3.5) |
| 6 | 13, 15, 16 | (124, 72) | exit rules |
| 8 | 2, 4, 6, 8 / 7, 9 / 3, 5, 10 / **11** | (1, 122) / (1, 123) / (1, 124) / **(1, 123)** | exit rules; **slot 11's rule tests (1, 122)** (OB-036) |
| 9 | 5 | (177, 169) | exit rule |

### 3.4 Message resources

| Resource | Address | File offset | Count | Used by |
|---|---|---|---:|---|
| `e2.flight_messages.01` (`M1`) | `2236:070F..0ADA` | 0x26BEF..0x26FBA | 11 | `Msn_UpdateA` |
| `e2.flight_messages.02` (`M2`) | `2236:1D2A..2409` | 0x2820A..0x288E9 | **20** | `Msn_UpdateB` (§5.10–§5.20) |
| `e2.objectives` | `0E96:59D7..` | 0x184B7.. | 20 | `Msn_Setup`: flight *n* draws element *n* − 1 (checked for 1–9) |

Both message runs are contiguous Pascal strings that end exactly where the next routine starts. `M1[1]` and `M2[10]` are the same "incoming transmission" text. Element addresses of `M1`: 070F, 0767, 07BF, 0817, 0871, 08CB, 0923, 097B, 09D3, 0A2B, 0A83.

## 4. Behaviour

### 4.1 Where mission logic runs in a tick

Step numbers are the rows of [tick-order](tick-order.md) §3, as used by [episode-deltas](../../re/episode-deltas.md) §4.1.

```text
step 6   projectiles and hits: a kill sets armour < 1 but leaves the class byte alone
         (episode-deltas 5.1, P3); the AI pass of step 10 removes the wreck
step 7   player draw (late-draw flights run the same code at step 16; steps 8-15 always run)
step 8   Msn_UpdateA (2236:0ADB): only the block of the current flight (1..9) runs      section 5
step 9   Msn_UpdateB (2236:240A): only the block of the current flight (10..20) runs    §5.10–§5.20
step 10  AI_UpdateShips (2D26:0011): pass 1 removes ships with armour < 1 or with the exit
         marker as waypoint; pass 2 moves live ships (ships spawned in steps 8-9 move this
         tick) and removes ships at the map edge. No flight-specific rules in E2
step 15  comm viewer: if g_CommRequest = 1 and back page = 0: g_CommRequest := 0;
         UI_CommViewer (0E96:83F9)                                                      4.6
step 18  (E1 Msn_UpdateEvents) removed: no respawns, no scripted events here
step 19  instruments (unchanged routine): ticker step, F6 indicator, guidance line
step 24  Esc flight menu
new      Msn_E2FlightEvents (0E96:99B8), called every tick at 0E96:D2D4, after the
         Esc menu and before the shield checks of step 26; acts only in flights 17, 18   4.7
step 27  player move: at the map limit, when the ticker is idle, g_EdgeHit := 1
         (the next step 19 posts the jurisdiction warning and restarts the ticker)
step 28  after the BIOS keyboard flush: F6 key byte = 1 and not over -> g_CommRequest := 1
step 31  player specials: M key and ticker idle -> ticker := 1 (replays the strip)
step 33  leave the loop if aborted, end ticks = 35, lost, or succeeded (0E96:EA1C)
teardown outcome tests                                                                   4.8
```

Consequences:

- **Kills are seen one tick late.** A ship destroyed by a projectile at step 6 of tick *t* is still live for every rule of step 8 in tick *t* (it can still be ordered to attack and still be a target, and "dead" tests ignore it). It is FFh from step 10 of tick *t*, so phase steps that wait for it fire at step 8 of tick *t* + 1.
- **Exit rules are seen one tick late by earlier rules.** A rule placed before an exit rule in the same block sees the removal only in the next tick (example: flight 9, §7 vector 12).
- **Arrivals.** A ship reaches a tile in step 10 of tick *t*; exit and arrival rules notice it in step 8 of tick *t* + 1.
- **Success** is decided at step 8 and acted on at step 33 of the same tick. Because the base test is the first rule of most blocks, a phase that becomes 100 in a block produces `succeeded` = 1 at the next tick at the earliest.

### 4.2 Shape of one flight block

`Msn_UpdateA` tests `g_FlightNumber` against 1, 2, …, 9 in turn and runs only the matching block; nothing in a block changes the flight number. A block is a flat list evaluated top to bottom once per tick. Differences from E1 [mission-logic](mission-logic.md) §4.2:

- The base test is the first rule except in flight 6 (after three exit rules). Its form differs per flight (§4.4); flights 6 and 7 do not test the phase.
- There are no `else hide` branches and no `go` (path skip) actions in flights 1–9. Ships that join later (flight 8) are hidden by setup and spawned by the phase script.
- Loops are Pascal `for` loops with constant bounds (inclusive). One loop counts down (flight 8). Loop bodies may contain several rules; the spec keeps them nested as in the original because the last successful `AI_SetTarget` for a ship in a tick wins (the order decides the target).

### 4.3 Spawning, hiding and removal

As E1 [mission-logic](mission-logic.md) §4.3, with these E2 points:

- **Spawn** (flight 8 only in flights 1–9): the hidden ship appears at its mission-file start tile with offsets (16, 16) (hidden ships are not moved). Its waypoint is still path step 1 (E2 has no setup `AI_ChooseWaypoint` loop), so the AI picks the next waypoint at its first live tick (U-205).
- **Removal** has five sources: exit rules (step 8/9), the hide in flight 3's arrival rule, and three removals in `AI_UpdateShips`: armour < 1 (a destroyed ship; the kill itself was counted at step 6 when the player fired), waypoint = exit marker (50, 0) or (50, 1), and the map-edge rule ([mission-ships](mission-ships.md) §4.2.3). Only the destruction counts as a kill.
- **"All dead" tests** do not distinguish destroyed, escaped and never-spawned ships. Flights 4, 5 and 8 advance when their target or convoy ships **leave**, exactly as if they had been destroyed. Flights 3 and 9 tell the two apart only through the outcome tests (§4.8).
- **No respawns.** E2 has no rule that gives a dead slot a class again except the flight-8 spawns of slots that setup hid.

### 4.4 Base tests

`succeeded` is recomputed at every tick by the block of the current flight (set when the condition holds, cleared otherwise):

| Flight | `succeeded :=` | Phase condition | Position in block | Address |
|---:|---|---|---|---|
| 1, 2, 3 | viewX = 102 and viewY = 104 and subY ≤ 36 | phase = 100 | first | `0AF3`, `1133`, `1387` |
| 4 | viewX = 102 and viewY = 104 | phase = 100 | first | `1577` |
| 5 | 178 ≤ viewX ≤ 183 and viewY = 35 | phase = 100 | first | `1719` |
| 6 | viewY = 70 and 118 ≤ viewX ≤ 123 | **none** | after the exit rules | `1874` |
| 7 | viewX = 102 and viewY = 104 and subY ≤ 35 | **none** | first | `1901` |
| 8 | viewX = 7 (any viewY) | phase = 100 | first | `1A5B` |
| 9 | viewX = 87 and viewY = 28 and subY ≤ 31 | phase = 100 | first | `1C5D` |

All compares are signed 16-bit (viewX, viewY, subY are i16). subY ranges over 16..48 within a view tile, so "subY ≤ 36" accepts the upper 21 positions of the base cell. Flights 1–4 and 9 start inside their base cell (start offsets 32, 16), so those flights end as soon as the phase reaches 100 if the player has not moved. Flight 6 ends the moment the player reaches its base strip, whatever the phase (phase stays 1 in flight 6). Flight 7 ends when the player is back at (102, 104), whether or not the transmission was read. The base test is also evaluated during the end sequence (U-244).

### 4.5 Messages and transmissions

- `post` overwrites the ticker strip and restarts the scroll (ticker := 1), even if a message is still scrolling. The rules that must not interrupt a message test "ticker idle" first: in flights 1–9 only the transmission announcements (flights 1, 5, 7).
- "Ticker idle" can be delayed by the player: the M key restarts the ticker when it is idle (step 31), and touching the map limit posts the jurisdiction warning when the ticker is idle (steps 27/19). Both hold off the announcements.
- **Announcements are posted once.** Unlike most E1 flights, flights 1, 5 and 7 move to phase 2 in the same step that posts the announcement, so the message is not repeated; comm pending stays 1 until F6 is pressed in phase 2 or a rule clears it.
- Comm pending is cleared by the comm viewer on a match, by flight 1 when slot 1 is gone and by flight 7 when slot 2 is gone. Flight 5 never clears it (OB-037).

### 4.6 Comm viewer (`UI_CommViewer` `0E96:83F9`, tick-order step 15)

F6 sets `g_CommRequest` (DS:162B) every tick its key byte is set, unless the mission is over (`0E96:D7A3`). At step 15, when the request is set and the back page is 0, the request is cleared and the viewer checks this table, one test after the other in this order (each flight appears once, so at most one row matches). Only a matching (flight, phase) shows a transmission; otherwise the request is simply consumed. Comm pending is **not** tested: F6 in the right phase shows the transmission even if the indicator is off.

| Code order | Flight | In phase | Shows (`UI_CommScreen(n)`) | Text resources | Then phase := |
|---:|---:|---:|---:|---|---:|
| 1 | 1 | 2 | 1 | `e2.comms.01` | 3 |
| 2 | 7 | 2 | 3 | `e2.comms.03` | 100 |
| 3 | 11 | 1 | 4 | `e2.comms.04` | 2 |
| 4 | 19 | 2 | 5 | `e2.comms.05` | 3 |
| 5 | 15 | 2 | 6 | `e2.comms.06` + `.07` | 3 |
| 6 | 16 | 2 | 7 | `e2.comms.08` | 3 |
| 7 | 5 | 2 | 2 | `e2.comms.02` | 3 |

On a match, in this order: F6 key byte (DS:19C4) := 0; `UI_CommScreen(n)` (modal: no ticks run while it is open); comm pending := 0; phase := new phase. E1's marker side effect (E1 flight 14) is gone. The table matches [episode-deltas](../../re/episode-deltas.md) §6 (verified). Rows 3–6 belong to §5.10–§5.20 flights.

### 4.7 Per-flight event routine (`Msn_E2FlightEvents`, tick-order step "new", `0E96:D2D4`)

`Msn_E2FlightEvents` (`0E96:99B8`) runs every tick at `0E96:D2D4` (after the Esc flight menu, before the shield and destruction checks). It returns at once unless the flight is 17 or 18; its trigger `g_CutsceneRequest` (DS:31DE) is set by `Msn_UpdateB` (`2236:2D1B`, `2236:2F15`). It does nothing in flights 1–9.

#### 4.7.1 `Msn_E2FlightEvents` (`td2:0E96:99B8..9BBF`)

**When it runs.** It is called once per tick, unconditionally, from `Sim_FlyMission` at `0E96:D2D4`. The call comes after the pause wait, the page toggle and the Esc flight menu (E1 tick-order steps 22–24) and before the armour/shield checks (E1 step 26). It runs in every flight, on both pages and during the end sequence. Its stores are therefore seen by the end-of-flight test (E1 step 33) **in the same tick**: a `lost` set here ends the flight in that tick. Explosions it starts are first drawn in the next tick's explosion step.

**State.**

| Variable | Address | Type | Reset |
|---|---|---|---|
| trigger (`g_CutsceneRequest`) | DS:31DE | u8 | := 0 at flight start (`0E96:A7D4`). Set only by `2236:2D1B` (flight 17) and `2236:2F15` (flight 18); never cleared in flight |
| n (debris counter) | DS:11E8 | u16 | := 0 at flight start (`0E96:A774`, the first store of `Sim_FlyMission`) |

**Behaviour, in code order.**

```text
if flight = 18 and trigger ≠ 0 and n < 2:              (unsigned)
    n := n + 1
    spawnDebris()
if flight = 18 and n >= 1:
    n := n + 1
    spawnDebris()
    if n = 5:
        draw e2.flight_events (0E96:995E) at strip (0, 735), ink 37, with Txt_FlightDrawStringOpaque (0E96:45BB)
        ticker := 1
    if n >= 150:
        lost := 1
        Snd_PlayEffect(100, 90, 29, low byte viewX, low byte viewY, sfx device)
        hit flash (DS:11E4, u16) := 0
        for e = 0..255: Gfx_SetDacEntry(e, 63, 63, 63)            (32EB:1901; whole DAC white)
        for e = 0..255: flight palette entry e (DS:1304 + 3e .. +2) := (63, 63, 63)
if flight = 17 and trigger ≠ 0:
    lost := 1

spawnDebris():
    for i = 1..20 (explosion slots; 21..30 are never used here):
        if frame(i) = 8 (idle):
            Snd_PlayEffect(9, 90, 29, low byte viewX, low byte viewY, sfx device)
            offsetX(i) := Random(48)
            offsetY(i) := Random(48)
            tileX(i)   := 91 + Random(2)
            tileY(i)   := 28 + Random(2)
            frame(i)   := 1
            leave the loop                                  (only the first idle slot)
    (no idle slot among 1..20: nothing happens: no sound, no Random)
```

**RNG order** per debris explosion:

1. any `Random` calls of `Snd_PlayEffect` on the PC-speaker path ([sfx-policy](../audio/sfx-policy.md); device 3 only);
2. `Random(48)` for offset X;
3. `Random(48)` for offset Y;
4. `Random(2)` for tile X;
5. `Random(2)` for tile Y.

The final sound 100 may also draw on the PC speaker. These draws come after every draw made in steps 1–24 of the same tick (flight 13's script, the AI, the projectile sounds).

**Timeline** (trigger first seen in tick T; 20 explosion slots are enough, because an explosion is idle again 6 ticks after frame := 1):

| Tick | n after | Events |
|---|---:|---|
| T | 2 | Two debris explosions (lowest and next-lowest idle slot), two sounds 9 |
| T + 1, T + 2 | 3, 4 | One debris explosion each |
| T + 3 | 5 | Debris; base-destroyed text in the ticker, ticker := 1 |
| T + 4 .. T + 147 | 6..149 | One debris explosion per tick |
| T + 148 | 150 | Debris, then lost := 1, sound 100, hit flash 0, white DAC and white palette copy; the tick loop ends at its end-of-flight test, and teardown fades out |

That makes 150 debris explosions over 149 ticks, all at tiles (91..92, 28..29). This is next to the home point (92, 28), and the sound source is (90, 29).

**Flight 17.** `lost` is set in the same tick as the trigger, so no debris, sound or palette effect is produced.

**Text.** `e2.flight_events` and every M2 element contain no digit other than "6", so the digit-advance difference between `Txt_FlightDrawStringOpaque` and the mission glyph routine (hud §4.7) has no effect in E2.

**E1 counterpart.** `Msn_UpdateEvents` (`file0001:0E8B:9860`) is gone. E2 has none of its respawns, the base-hit counter, timers, the bomb, the ship switch or the cut-scene.

#### 4.7.2 U-200 resolved: what sets `g_CutsceneRequest` (E2 DS:31DE)

Both setters are now inside `Msn_UpdateB`:

1. **`2236:2D1B`, flight 17.** It sets the trigger when phase = 2 and the ticker is idle. Phase 2 is entered (rule B) when both transports (slots 2 and 3) are gone and neither has tile X = 1, that is, both were destroyed. The trigger is therefore set when the "both destroyed" message M2[15] has finished scrolling, 155 or 156 ticks after it was posted (hud §4.7; pressing M at the right moment can postpone it). `Msn_E2FlightEvents` then sets `g_MissionLost` := 1 in the same tick. The flight ends as **lost**, which leads to the lost-retry prompt and, on decline, ending C. **To avoid it, the player must get at least one transport to the south-west exit (1, 163)**; for outcome 1 both must get there (§5.17).
2. **`2236:2F15`, flight 18.** It sets the trigger when any of slots 6..11 stands on tile (92, 31), the base and the player's start tile. There is no class or phase test. `Msn_E2FlightEvents` then plays the 149-tick base destruction (§4.7.1) and sets `g_MissionLost` := 1 on its last tick. **To avoid it, the player must destroy all six Vulture IIIs before any of them reaches (92, 31).** The second wave (9..11) only enters when 6, 7 and 8 are gone.

E2 has no cut-scene. The flag keeps its E1 name, but in E2 it is only the trigger of `Msn_E2FlightEvents`.

### 4.8 End of flight and outcome (`0E96:EA1C..EB4B`)

```text
step 33: if aborted or end ticks = 35 or lost or succeeded: leave the tick loop
fade out (Gfx_FadeOutFlight); restore the keyboard handler; Crt text mode 3
g_PlayerDied := (end ticks = 35)
outcome := 1
flight 3:  outcome := (tile(1) = (84,135))
flight 16: outcome := not (tileX(2) = 177 and tileX(2) = 168)          (always 1, OB-025)
flight 8:  outcome := 0; for i = 2..11: if tileX(i) = 1: outcome := 1   (loop counter g_I, DS:197A)
flight 9:  if tile(5) = (177,169): outcome := 0
flight 10: if tile(2) = (151,199): outcome := 0
flight 11: if tileX(2) = 177 or tileX(6) = 177 or tileX(11) = 177: outcome := 0
flight 17: if not (tileX(2) = 1 and tileX(3) = 1): outcome := 0
```

The tests are in code order; each touches only its own flight. They read tiles only (class is ignored), so a ship removed at its exit tile counts as "arrived" and a ship destroyed elsewhere does not. E1's flight-17 rule "outcome := not lost" is gone. The flights 1–9 results: flight 3 is 1 only when the escorted truck arrived (§5.3); flight 8 is 1 when at least one of slots 2..11 (trucks or escorts) has tile X = 1 at teardown (normally a ship removed at its exit); flight 9 is 0 when the target truck escaped through (177, 169). Every other flight 1–9 gives 1. [episode-deltas](../../re/episode-deltas.md) §4.5 is verified for all seven tests.

#### 4.8.1 Flights 10–20 (from the `Msn_UpdateB` analysis)

Verified against episode-deltas §4.5; all rows match. Teardown order:

1. leave the loop if aborted, end ticks = 35, lost or succeeded;
2. `Gfx_FadeOutFlight`;
3. restore INT 09h;
4. `Crt` text mode 3;
5. `g_PlayerDied` := (end ticks = 35);
6. `g_MissionOutcome` := 1;
7. the per-flight tests below, in code order;
8. sound and music cleanup.

Each test only reads tiles (u8), which are kept after removal.

```text
flight 3:  outcome := (tile(1) = (84,135))                                          (part A)
flight 16: if tileX(2) = 177 and tileX(2) = 168: outcome := 0 else outcome := 1    (OB-025: always 1)
flight 8:  outcome := 0; for s = 2..11: if tileX(s) = 1: outcome := 1              (part A)
flight 9:  if tile(5) = (177,169): outcome := 0                                     (part A)
flight 10: if tile(2) = (151,199): outcome := 0                                     (OB-040: unreachable)
flight 11: if tileX(2) = 177 or tileX(6) = 177 or tileX(11) = 177: outcome := 0
flight 17: if not (tileX(2) = 1 and tileX(3) = 1): outcome := 0
```

| Flight | Meaning | Note |
|---:|---|---|
| 10 | Meant to test slot 2 (the escorted ship) at its path end (151, 199) | The exit rules remove slot 2 on row 198 first (§5.10), so the tile cannot be (151, 199): **outcome is 1** whether slot 2 escaped or was destroyed (OB-040) |
| 11 | 0 if any convoy truck escaped through an east exit (X = 177) | Destroyed trucks never have X = 177 unless killed in that column |
| 16 | Meant: 0 if Kane escaped at (177, 168) | It compares tile X twice (177, then 168), so the outcome is always 1 (OB-025, confirmed) |
| 17 | 1 only if both transports ended in column 1 (their exit is (1, 163)) | Unlike E1 (outcome := not lost), E2 tests tiles. A lost flight 17 also has outcome 0 |
| 12–15, 18–20 | Always 1 | |

### 4.9 Presentation and audio hooks

Flights 1–9 emit only ticker posts (`post`, colours 37 and 42) and the comm-pending flag, which drives the F6 indicator and its "income" sound 200 ([hud](../render/hud.md) §4.8), and the comm screens 1–3 through F6. Mission logic of flights 1–9 plays no sound and changes no palette.

## 5. Per-flight logic

Each section lists the block in original order. Messages use colour 37 unless a step says `c = 42`.

| Flight | Phases used | Transitions |
|---:|---|---|
| 1 | 1, 2, 3, 100 | 1 → 2: player near slot 1 (B); 2 → 3: F6 (comm 1); any → 100: slot 1 gone (A) |
| 2 | 1, 100 | slots 4..8 and 12..18 all gone |
| 3 | 1, 100 | escorted truck arrived (A) or gone (B) |
| 4 | 1, 100 | convoy 1..3 gone |
| 5 | 1, 2, 3, 100 | 1 → 2: first idle ticker (B); 2 → 3: F6 (comm 2); any → 100: slots 1..3 gone (A) |
| 6 | 1 | never changes; success is positional |
| 7 | 1, 2, 100 | 1 → 2: first idle ticker (B); 2 → 100: F6 (comm 3); the phase does not affect success |
| 8 | 1, 2, 3, 100 | three waves (A, B, C) |
| 9 | 1, 100 | slot 5 gone (A) |

### 5.1 Flight 1 (`2236:0AE9..1128`)

Start: §3.3. Slot 1 is the target (a class-2 ship, marker 32h); slots 3, 4 are friendly class-4 ships without markers; slots 6..13 are enemies. Comm viewer: phase 2 → transmission 1, phase 3.

```text
succeeded := view = (102,104) and subY ≤ 36 and phase = 100
A: if dead(1) and phase < 100: phase := 100; post(M1[0])
B: if ticker idle and phase = 1 and Msn_NearLive(1, player, 15):
       post(M1[1], c = 42); comm pending := 1; phase := 2
C: if dead(1): comm pending := 0
engage 1 → 6, 7, 8, 9, 15
engage 1 → player, 35
engage 3 → 6, 7, 8, 9, 10, 11, 12, 13, 20
engage 4 → 6, 7, 8, 9, 10, 11, 12, 13, 15
engage 6 → 1, 3, 4, 15;   engage 6 → player, 20
engage 7 → 1, 3, 4, 15;   engage 7 → player, 20
engage 8 → 3, 4, 1, 15;   engage 8 → player, 15       (target order 3, 4, 1)
engage 9 → 1, 3, 4, 15;   engage 9 → player, 20
engage 10 → 1, 3, 4, 15;  engage 10 → player, 15
engage 11 → 1, 3, 4, 15;  engage 11 → player, 20
engage 12 → 1, 3, 4, 15;  engage 12 → player, 20
engage 13 → 1, 3, 4, 15;  engage 13 → player, 20
```

Notes:
- The transmission is announced only when the player comes within 15 tiles (player tile convention, §2) of a live slot 1 while the ticker is idle. A and B cannot both fire in one tick (A sets phase 100).
- The flight completes when slot 1 is gone, whether or not the transmission was announced or read. If slot 1 dies in phase 2, C clears the indicator and F6 no longer matches (phase 100).
- Nothing requires the enemies 6..13 to be destroyed.
- Outcome = 1.

### 5.2 Flight 2 (`2236:1129..137C`)

Start: §3.3. Slots 1, 2 are friendly class-2 ships; slots 4..8 (class 3) and 12..18 (class 7) are the targets (markers 32h) and also fight each other.

```text
succeeded := view = (102,104) and subY ≤ 36 and phase = 100
for s = 4..18: engage s → player, 15
A: if dead(4..8, 12..18) and phase < 100: post(M1[2], c = 42); phase := 100
for b = 4..8:  engage 1 → b, 15;  engage b → 1, 15
for b = 4..8:  engage 2 → b, 15;  engage b → 2, 15
for b = 12..18: engage 2 → b, 15;  engage b → 2, 15
for a = 4..8:
    for b = 12..18: engage a → b, 15;  engage b → a, 15
```

Notes:
- A is computed with a local flag: cleared, then set if any of slots 4..8 or 12..18 is live. Slots 9..11 are empty and not tested.
- Slot 1 is not paired with 12..18 (the original's first loop covers attacker 1 only against 4..8).
- Slot 8 has class 3 from setup although the mission file has no ship there; it sits at tile (0, 0) and is removed by the map-edge rule of `AI_UpdateShips` in step 10 of the first tick, before any rule can see it as the last live target (U-243). Its marker never draws.
- Outcome = 1.

### 5.3 Flight 3 (`2236:137D..156C`)

Start: §3.3; palette mode 1 (infra-red). Slot 1 is a class-0 truck to be escorted (marker 39h); its path ends at (84, 135). Slots 3..8 are enemies.

```text
succeeded := view = (102,104) and subY ≤ 36 and phase = 100
engage 3 → 1, 7;        engage 3 → player, 10
engage 4 → player, 15;  engage 4 → 1, 10
engage 5 → player, 10;  engage 5 → 1, 7
engage 6 → player, 10;  engage 6 → 1, 10
engage 7 → 1, 10;       engage 7 → player, 20
engage 8 → player, 30;  engage 8 → 1, 7
A: if tileX(1) is 85, 86 or 87 and tileY(1) = 135 and class(1) ≠ FFh:
       hide 1; post(M1[3]); tileX(1) := 84; tileY(1) := 135; phase := 100
B: if dead(1) and phase < 100: post(M1[4]); phase := 100
```

Notes:
- The truck's path enters the strip at (87, 135), three tiles before its end, so A fires at step 8 of the tick after the truck first stands on (87, 135) (or on 86/85 if it skipped a tile). A then moves the truck's tile to (84, 135), which is what the outcome test looks for.
- The engagement rules run before A, so in the tick of the arrival enemies can still be ordered to attack slot 1.
- B fires when the truck is destroyed (one tick after the kill, §4.1); A's hide does not trigger B (phase is already 100).
- Outcome (§4.8): 1 when the truck arrived, 0 when it was destroyed (unless it was destroyed exactly on (84, 135)).

### 5.4 Flight 4 (`2236:156D..170E`)

Start: §3.3. Slots 1..3 are a class-0 convoy (markers 32h) heading for the west exit (1, 122); slots 8..11 (class 7) and 14..16 (class 3) are two hostile groups.

```text
succeeded := view = (102,104) and phase = 100                (no sub-tile test)
A: if dead(1, 2, 3) and phase < 100: post(M1[5]); phase := 100
exit 1 @ (1,122);  exit 2 @ (1,122);  exit 3 @ (1,122);  exit 8 @ (1,122);  exit 9 @ (1,122)
exit 10 @ (1,123); exit 11 @ (1,122); exit 14 @ (1,122); exit 15 @ (1,122); exit 16 @ (1,123)
for a = 8..11:
    for b = 14..16:
        engage a → b, 15;  engage b → a, 15;  engage a → player, 15
for a = 14..16:
    for b = 1..3: engage a → b, 20
```

Notes:
- The player rule sits inside the inner loop, so it is evaluated three times per attacker; when the player is in range it is the last successful call for that attacker in the tick, so the player wins over slots 14..16 as its target.
- The convoy counts as dealt with when it leaves at (1, 122) (one tick after it arrives, the exit rule removes it; A fires one tick later). The flight can be completed without a shot.
- Slot 9's path never reaches (1, 122); its exit rule is inert.
- Outcome = 1.

### 5.5 Flight 5 (`2236:170F..1845`)

Start: §3.3; player class 7. Slots 1..3 are class-0 transports (targets), slots 5..9 class-3 escorts, slot 11 a class-7 ship that fights the escorts and the transports. Comm viewer: phase 2 → transmission 2, phase 3.

```text
succeeded := 178 ≤ viewX ≤ 183 and viewY = 35 and phase = 100
A: if dead(1, 2, 3) and phase < 100: post(M1[6]); phase := 100
B: if ticker idle and phase = 1: post(M1[1], c = 42); comm pending := 1; phase := 2
for a = 5..9:
    engage a → 11, 20;  engage a → player, 15;  engage 11 → a, 15
for b = 1..3: engage 11 → b, 20
```

Notes:
- B fires at the first tick whose step 8 sees the objective ticker finished (§7 vector 14).
- The transports' paths end at the AI exit marker (50, 1): if they reach it they are removed and A completes the flight as if they had been destroyed (no outcome test).
- Comm pending is never cleared by a rule: if the transports are gone before F6 is pressed, the indicator keeps pulsing for the rest of the flight and F6 no longer matches (OB-037).
- Outcome = 1.

### 5.6 Flight 6 (`2236:1846..18F6`)

Start: §3.3; player class 7; no markers (the guidance line points home, (125, 68)).

```text
exit 13 @ (124,72);  exit 15 @ (124,72);  exit 16 @ (124,72)
succeeded := viewY = 70 and 118 ≤ viewX ≤ 123                (no phase test)
for a = 2..11:
    for b = 13..16: engage a → b, 15
    engage a → player, 20
```

Notes:
- There is no phase script; the phase stays 1. The flight ends as soon as the player's view reaches the strip (118..123, 70), whatever happened to the ships.
- Slots 13, 15 and 16 leave at their path end (124, 72). Slot 14 is empty.
- Outcome = 1.

### 5.7 Flight 7 (`2236:18F7..1A50`)

Start: §3.3; player class 3; no markers (guidance points home, (107, 104)). Comm viewer: phase 2 → transmission 3, phase 100.

```text
succeeded := view = (102,104) and subY ≤ 35                  (no phase test)
A: if dead(2): comm pending := 0
B: if ticker idle and phase = 1: post(M1[1], c = 42); comm pending := 1; phase := 2
for a = 5..13: engage a → player, 15
```

Notes:
- The sub-tile limit is 35 here (36 in flights 1–3).
- A precedes B: if slot 2 is already gone when B fires, the flag is set after the clear, so the indicator lights for that one tick (with sound 200) and is cleared at the next tick's A (OB-037). F6 in phase 2 still shows transmission 3.
- The transmission sets phase 100, but the phase does not affect success: flying back to (102, 104) ends the flight in any phase.
- Slot 2 is read only by A; slots 3 and 4 have no rules.
- Outcome = 1.

### 5.8 Flight 8 (`2236:1A51..1C52`)

Start: §3.3; player class 3 with late draw (map collision skipped, [episode-deltas](../../re/episode-deltas.md) §4.2); objective colour 42. Three convoys leave the hangar at (107, 106) for the west edge: wave 1 = trucks 2, 3 and escort 4 (live at start), wave 2 = trucks 5, 6 and escort 7, wave 3 = trucks 8, 9, 10 and escort 11 (hidden at start). Slots 15..19 are enemies. Markers: 1..11 = 39h, 15..20 = 32h.

```text
succeeded := viewX = 7 and phase = 100                        (any viewY)
for a = 15..19:
    for b = 11 down to 2: engage a → b, 20
    engage 4 → a, 20;  engage 7 → a, 20;  engage 11 → a, 20
A: if dead(2, 3) and phase = 1:
       post(M1[7], c = 42); phase := 2; spawn 5 as 0; spawn 6 as 0; spawn 7 as 4
B: if dead(5, 6) and phase = 2:
       post(M1[8], c = 42); phase := 3; spawn 8 as 0; spawn 9 as 0; spawn 10 as 0; spawn 11 as 4
C: if dead(11, 10, 9, 8) and phase = 3: phase := 100; post(M1[9], c = 42)
exit 2 @ (1,122);  exit 3 @ (1,124);  exit 4 @ (1,122);  exit 5 @ (1,124);  exit 6 @ (1,122)
exit 7 @ (1,123);  exit 8 @ (1,122);  exit 9 @ (1,123);  exit 10 @ (1,124); exit 11 @ (1,122)
```

Notes:
- No rule orders any ship to attack the player in this flight; the enemies attack the convoys and the escorts 4, 7, 11 attack the enemies. The inner loop runs from 11 down to 2, so when several convoy ships are in range the lowest-numbered one is the enemy's final target in that tick. Slot 20 is empty.
- Waves 1 and 2 need only their trucks gone; wave 3 also needs its escort (slot 11) gone.
- **Slot 11 never leaves through its exit rule:** its path ends at (1, 123) but the rule tests (1, 122). At the path end the AI wraps the path, so slot 11 flies back east and loops its route; phase 3 waits until it is destroyed (OB-036, U-242).
- Spawned ships appear at the hangar tile (107, 106), not near the player.
- Outcome (§4.8): 1 when at least one of slots 2..11 is on column 1 at teardown (normally a truck removed at its exit), else 0.

### 5.9 Flight 9 (`2236:1C53..1D25`)

Start: §3.3. Slot 5 is the class-0 target truck (marker 32h), from (1, 30) to the exit (177, 169); slot 2 is a friendly class-4 ship; slots 8..15 are enemies; slot 3 (class 4) has no rules.

```text
succeeded := view = (87,28) and subY ≤ 31 and phase = 100
A: if dead(5) and phase = 1: phase := 100; post(M1[10], c = 42)
exit 5 @ (177,169)
for a = 8..15:
    engage a → 2, 15;  engage a → player, 30;  engage 2 → a, 15
```

Notes:
- A precedes the exit rule: when the truck reaches (177, 169), the exit rule removes it at step 8 of the next tick and A fires one tick after that (§7 vector 12).
- Phase 100 is reached whether the truck was destroyed or escaped; the outcome (§4.8) is 0 when it escaped.
- The flight starts inside its base cell (view (87, 28), subY 16), so if the player stays put the flight ends one tick after phase 100.
- Nothing requires the enemies 8..15 to be destroyed.

### 5.10.0 Conventions and start state for flights 10–20

#### 5.10.0.1 Slot numbering in the td2 listings (trap)

The listing annotations for the per-slot arrays are **0-based**. They are not slot numbers: `[0x39a3] g_SlotClass[1]` is **slot 2**. The code addresses the arrays as follows (checked against `Msn_ExitAt` `2236:05DC`):

| Array | Element of slot s | Notes |
|---|---|---|
| class | DS:39A1 + s | `g_SlotClass` (td2.csv gives DS:39A2 = slot 1) |
| tile X / tile Y | DS:3A55 + s / DS:3A69 + s | |
| heading / target heading | DS:39C9 + s / DS:39DD + s | |
| marker | DS:387F + s | The home marker (element 21) is DS:3894 |
| fire request (s, e) | DS:3B14 + 9s + e, e = 1..9 | E1 DS:3BEC + 9s + e (mission-ships §2) |
| ship record field f | DS:435A + (s − 1) × 0429h + f | armour i16 at f = 3FAh, aggression u8 at f = 3F6h |
| explosion slot i (1..30): frame, tile X, tile Y, offset X, offset Y | DS:38C1 + i, 38DF + i, 38FD + i, 391B + i, 3939 + i | frame 8 = idle (structures §5) |

#### 5.10.0.2 Notation added to E1 §3.2

| Notation | Meaning |
|---|---|
| `post(M2[k], c)` | `Msn_PostTicker(0, 735, M2[k], c)`, then ticker := 1. **The colour is always written out in this part.** Most E2 posts use 42; the E1 default of 37 does not apply here |
| `M2[k]` | `e2.flight_messages.02` element k (0-based; strings at `2236:1D2A..2409`, **20 elements**). Flights 10–20 use only M2. M1 (`e2.flight_messages.01`, `2236:070F..0ADA`, 11 elements) belongs to `Msn_UpdateA` |
| `engage a → b, r` | `if Msn_NearLive(a, b, r) then AI_SetTarget(a, b)`. `Msn_NearLive` is symmetric in its first two arguments, so the original's argument order of the range test does not matter. `AI_SetTarget` (`2F63:3232`) is the routine that [mission-ships](mission-ships.md) §4.3.10 calls `AI_OrderAttack` |
| player (as a slot) | slot 0 in `Msn_NearLive`. E2 uses the tile (low byte (viewX + 5), low byte (viewY + 3)) (RE-9b's part). At the E2 start view (87, 28) this is (92, 31) |
| `base(87, 28)` | `succeeded := (viewX = 87 and viewY = 28 and viewSubY < 32 and phase = 100)`, else succeeded := 0. These are i16 compares (viewSubY is signed). viewSubX is not tested. View offsets live in 16..48, so the player must be in the upper half (sub-Y 16..31) of the start cell. Flights 10–19 use exactly this test. Flight 20 has its own (§5.20) |
| `trigger` | `g_CutsceneRequest`, DS:31DE, u8. In E2 it only arms `Msn_E2FlightEvents` (§4.7.1); E2 has no cut-scene |
| `fire(2, h)` | fire request (2, h) := 1 |
| `allGone` | scratch byte DS:31C5 (`g_MsnEventCount`), recomputed in the tick that uses it |
| `ticker idle` | `g_TickerPos` (DS:31E4) = 0. `ticker = 1` means "restarted since the last page-0 ticker step" (hud §4.7) |

There are no marker writes and no armour writes in `Msn_UpdateB` for flights 10–20. The only `Random` call is in flight 13.

#### 5.10.0.3 Start state of flights 10–20 (`Msn_Setup` `0E96:60F9`, blocks `66E8..6D51`)

The table is given so the rules can be checked. Every block clears markers 1..20, sets its own markers and sets home marker := 3Dh (flight 11 does its hides and aggression writes before the marker clear). The objective is `e2.objectives[n − 1]`, drawn with `Txt_FlightDrawStringOpaque` (`0E96:45BB`) at (0, 735), and ticker := 1. Phase := 1. Armour and aggression are ship-record writes (B.0.1). They happen after the class-table initialisation at `0E96:B316..B359` (armour = 2 × `k_ClassArmour[class]`, DS:0426 + class = 31, 16, 15, 20, 25, 25, 31, 25, 20, 0 for classes 0..9), because `Msn_Setup` is called later, at `0E96:B6E1`.

| Flight | Block | Objective colour | Player class | Markers 32h (target) / 39h (friend) | Hidden (class FFh) | Other |
|---:|---|---:|---:|---|---|---|
| 10 | `66E8..679D` | 37 | 2 | 32h: 14..16; 39h: 2 | 14..16 | palette mode := 1 (infra-red); armour(2) := 500; armour(8) := 250; aggression(4..7) := C3h, aggression(15) := C3h, aggression(16) := B4h |
| 11 | `679E..6835` | 37 | 5 | 32h: 2, 6, 11 | 6..14 | **comm pending := 1**; aggression(2..14) := C8h |
| 12 | `6836..68B6` | 42 | 5 | 39h: 2..4 | — | aggression(8..13) := C8h |
| 13 | `68B7..6972` | 42 | 5 | 32h: 4..6, 8..11, 13..15 | — | aggression(4..15) := E1h |
| 14 | `6973..6A17` | 42 | 2 | 32h: 4..8, 10..12 | — | aggression(4..16) := C3h; aggression(2) := FFh |
| 15 | `6A18..6A86` | 42 | 2 | 32h: 2 | — | aggression(4..7) := 7Dh; aggression(2) := C8h |
| 16 | `6A87..6AFC` | 42 | 2 | 32h: 2 | — | aggression(5..12) := E1h; armour(2) := 500; armour(10) := 200 |
| 17 | `6AFD..6BBF` | 42 | 2 | 39h: 2, 3 | — | aggression(5, 6) := 0Fh; class(12) := 6; armour(2) := 5 × armour(2), armour(3) := 5 × armour(3) (16-bit; 160 each for the file class 1); aggression(8..14) := 7Dh |
| 18 | `6BC0..6C3E` | 42 | 2 | 32h: 6..11 | 9..11 | — |
| 19 | `6C3F..6CAE` | 42 | 2 | 32h: 2 | — | aggression(2..7) := FFh; armour(2) := 1000 |
| 20 | `6CAF..6D50` | 42 | 5 | 32h: 12..15 | — | aggression(2..11) := 0Ah; aggression(12..15) := DCh |

#### 5.10.0.4 Comm viewer rows for flights 10–20 (`UI_CommViewer` `0E96:83F9`)

These are the same rule as E1 §4.6. When the F6 request is consumed with the matching (flight, phase): F6 key byte DS:19C4 := 0, `UI_CommScreen(n)`, comm pending := 0, phase := new phase. The tests in code order are (1,2), (7,2), (11,1), (19,2), (15,2), (16,2), (5,2). No case can chain into another.

| Flight | In phase | Screen n | Then phase |
|---:|---:|---:|---:|
| 11 | 1 | 4 | 2 |
| 15 | 2 | 6 | 3 |
| 16 | 2 | 7 | 3 |
| 19 | 2 | 5 | 3 |

Flights 10, 12, 13, 14, 17, 18 and 20 have no transmission.

---

### 5.10 Flight 10 (infra-red; escort slot 2 south; then a convoy from the east)

`Msn_UpdateB` `2236:2414..25EC`. Roster (`m10.td2`):

- slot 2: class 1, (98, 32) → path ends (151, 199) via (151, 198);
- slot 3: class 2, friendly;
- slots 4, 6, 7, 8: class 7, and slot 5: class 8, patrols;
- slots 14 (class 7), 15 and 16 (class 0): start at (199, 99..101) and end at the east exits (177, 175) / (177, 167) / (177, 169).

Slot 1 is empty.

```text
base(87, 28)
A: if phase = 1 and ticker idle:   post(M2[0], 42); phase := 18
B: if dead(14) and phase = 2:      post(M2[1], 42); phase := 100
C: if dead(2) and phase = 18:      post(M2[2], 42); phase := 2; spawn 14 as 7; spawn 15 as 0; spawn 16 as 0
for s = 4..8:  engage s → 3, 15;  engage 3 → s, 15;  engage s → player, 15;  engage s → 2, 20
for s = 14..16: engage s → player, 15
exit 2 @ (149,198); exit 2 @ (151,198); exit 2 @ (150,198); exit 2 @ (152,198); exit 2 @ (153,198)
exit 14 @ (177,175); exit 15 @ (177,167); exit 16 @ (177,169)
```

Notes:

- **Phase 18 is a real value.** The first announcement moves the phase from 1 to 18 (12h), not to 2. B is tested before C, so the tick in which C sets phase 2 and spawns slot 14 cannot also complete B: slot 14 is live from that tick on.
- Slot 2 only has to be **gone** (removed at its exit row 198, or destroyed) for C. Only slot 14 has to be gone for B, and trucks 15 and 16 are optional. Outcome: §4.8.1 and OB-040.
- Comm: none in E2.

### 5.11 Flight 11 (transmission; three convoys to the east exits)

`2236:25ED..27E7`. Roster (`m11.td2`):

- convoy trucks (class 0): slot 2, (149, 1) → (177, 169); slot 6, (1, 30) → (177, 167); slot 11, (199, 100) → (177, 169);
- escorts: slot 3 (class 8) and slot 4 (class 7), which start with slot 2 and end at (151, 199) / (150, 199);
- slots 7, 8, 9 (class 7): end at (151, 199), (149, 1) and (177, 167);
- slot 12 (class 7): (199, 100) → (177, 169).

Setup hides 6..14 and sets comm pending := 1. Comm viewer: phase 1 → screen 4, phase 2.

```text
base(87, 28)
A: if phase = 2:                         post(M2[3], 42); phase := 3
B: if dead(2) and phase <= 3:            phase := 4; post(M2[4], 42); spawn 6 as 0; spawn 7 as 7; spawn 8 as 7; spawn 9 as 7
C: if dead(6) and phase = 4:             phase := 5; post(M2[5], 42); spawn 11 as 0; spawn 12 as 7
D: if dead(11) and phase = 5:            phase := 100; post(M2[6], 42); class(11) := 0; class(12) := 7
exit 2 @ (177,169); exit 3 @ (151,199); exit 4 @ (150,199); exit 6 @ (177,167); exit 7 @ (151,199)
exit 8 @ (149,1);   exit 9 @ (177,167); exit 11 @ (177,169); exit 12 @ (177,189)
for s = 3, 4:  engage s → player, 15
for s = 7..9:  engage s → player, 15
engage 12 → player, 15
```

Notes:

- The transmission is announced by setup (comm pending = 1 from the first tick, and the objective text asks for F6). Reading it (phase 1 → 2) makes A post the first-convoy message in the next tick, with no ticker test.
- **B fires in phases 1, 2 or 3.** If the first convoy truck (slot 2) is gone before F6 is pressed, the phase jumps to 4. Transmission 4 can then never be shown, and nothing clears comm pending: the F6 lamp keeps animating for the rest of the flight (hud §4.8). The sound 200 only plays when the lamp phase is 0, so it is heard once. Preserve.
- **D repeats C's spawn** (`2236:26E2..26E6`), in our reading a copy-paste:
  - If slot 11 reached its exit (177, 169), the respawned truck is removed again by `exit 11` later in the same block.
  - If it was destroyed, its armour is < 1, and `AI_UpdateShips` pass 1 removes it again in step 10 without a kill or an explosion.
  - Slot 12 is set to class 7 again. Its exit (177, 189) is not on its path (which ends at (177, 169)), so a live slot 12 is never removed by the script (U-248).
  - The repeat has no lasting effect. Preserve the order anyway (T1).
- There are no ship-against-ship engagements.
- Outcome: 0 if any convoy truck (2, 6, 11) ends with tile X = 177, that is, if it escaped (§4.8.1).

### 5.12 Flight 12 (escort three trucks north to the base)

`2236:27E8..28A6`. Roster (`m12.td2`): trucks 2, 3, 4 (class 0) start at (75, 197/195/193) and end at (92, 29); Vultures 8..13 (class 7) patrol.

```text
base(87, 28)
for s = 8..13: engage s → player, 15
A: if dead(2, 3, 4) and phase < 100: phase := 100; post(M2[7], 42)
exit 2 @ (92,29); exit 3 @ (92,29); exit 4 @ (92,29)
```

The Vultures only engage the player; no rule sends them against the trucks. Trucks that are destroyed count as gone, exactly like trucks that arrive, and there is no outcome test. Flight 12 cannot be failed except by dying.

### 5.13 Flight 13 (the malfunctioning Areo-1; patrol)

`2236:28A7..2952`. Roster (`m13.td2`): slot 2 is class 5 (the Areo-1, a short 21-step loop at (93, 37)); slots 4..6, 8, 10, 11, 14, 15 are class 7; slots 9 and 13 are class 8.

```text
base(87, 28)
targetHeading(2) := Random(9)                         (every tick, whatever class(2) and the phase are)
if heading(2) < 10: fire(2, heading(2))
A: if ticker idle and phase = 1: phase := 100; post(M2[8], 42)
for s = 4..15: engage s → player, 15
```

Notes:

- **RNG:** exactly one `Random(9)` per tick in step 9 for the whole flight. This is the only `Random` in `Msn_UpdateB`.
- The random target heading is used by the turn step (d) of `AI_UpdateShips` in the same tick ([mission-ships](mission-ships.md) §4.2, step (d)). That step only compares target ≠ heading and then steps the ring in the stored turn direction, so a value of 0 (not a heading code) behaves like any other mismatch. The AI's own target-heading choice in step (e) then overwrites the value. The visible effect is a ship that jitters as it turns.
- The fire request makes slot 2 fire a volley along its current heading every second tick (back page ≠ 0, tick-order step 30) whenever the heading is in 2..8. The spawn loop only scans entries e = 2..8: entries 1 and 9 are cleared without firing, and two-digit headings raise no request.
- **The Areo-1 keeps firing after it is gone** (OB-039). Neither the rule nor the volley spawner (`0E96:E05D..E23B`) tests the class. See §6.
- The flight's only objective is to wait out the objective text: phase 100 comes about 156 ticks after the start, and then the player returns to base. The marked ships are optional.

### 5.14 Flight 14 (Vultures in the south-east)

`2236:2953..2A51`. Roster (`m14.td2`): slot 2 is class 2 (a friendly ship, aggression FFh); slots 4..8 are class 7; slots 10..12 are class 6. Slots 9 and 13 are empty.

```text
base(87, 28)
A: if ticker idle and phase = 1 and dead(4, 5, 6, 7, 8, 10, 11, 12): phase := 100; post(M2[9], 42)
for s = 4..13: engage s → 2, 15;  engage 2 → s, 15;  engage s → player, 20
```

When the last target dies while a message is scrolling, A waits for the ticker to go idle. Slot 2 does not have to survive.

### 5.15 Flight 15 (the Areo-2)

`2236:2A52..2B4A`. Roster (`m15.td2`):

- slot 2: class 5 (the Areo-2), with a 331-step patrol;
- slots 4 and 5: class 6;
- slots 6 and 7: class 7;
- slots 10 and 11: class 1.

Comm viewer: phase 2 → screen 6, phase 3.

```text
base(87, 28)
A: if phase = 1 and ticker idle: post(M2[10], 42); comm pending := 1; phase := 2
B: if dead(2): comm pending := 0
C: if dead(2) and phase < 100: phase := 100; post(M2[11], 42)
for s = 10..12: engage 2 → s, 25
for s = 2..7:   engage s → player, 15
```

- The announcement is posted once, when the objective text has finished, and is not repeated.
- Destroying slot 2 completes the flight in phase 1, 2 or 3; the transmission is optional.
- If slot 2 dies in phase 1, A no longer fires.

### 5.16 Flight 16 (Kane runs for the south-east)

`2236:2B4B..2C68`. Roster (`m16.td2`):

- slot 2: class 1 (Kane), (93, 37) → (177, 168), armour 500;
- slots 5 and 6: class 7;
- slot 7: class 6;
- slots 8 and 9: class 8;
- slot 10: class 8, armour 200, at (148, 150).

Comm viewer: phase 2 → screen 7, phase 3.

```text
base(87, 28)
exit 2 @ (177,168)
A: if dead(2) and tile(2) = (177,168) and phase < 100: phase := 100; post(M2[12], 42)      (escaped)
B: if dead(2) and phase < 100:                          phase := 100; post(M2[13], 42)      (killed)
C: if Msn_NearLive(player, 10, 25) and phase = 1 and ticker = 1:
       post(M2[10], 42); comm pending := 1; phase := 2
D: if dead(10): comm pending := 0
for s = 5..10: engage s → player, 15
```

Notes:

- A and B both end the script; the flight completes either way. The intended "Kane escaped → failure" outcome test is broken (OB-025), so the outcome is always 1 (§4.8.1).
- **C tests `ticker = 1`, not `ticker idle`** (OB-038). The value is 1 only between a ticker restart and the next page-0 ticker step:
  - at the first tick (setup sets 1);
  - after a post;
  - after the map-edge warning;
  - in the tick after the player presses M while the ticker is idle (weapons §4.1; M is handled at step 31, after step 9).

  In phase 1 no rule of this flight posts. So the transmission appears only if the player stands within 25 tiles of slot 10 on both axes (player tile = view + (5, 3)) and the ticker has just been restarted, in practice by pressing M. Slot 10 starts at (148, 150), far from the start tile (92, 31). The transmission is optional; nothing else depends on phases 2 and 3.

### 5.17 Flight 17 (clear the way for two transports to the south-west exit)

`2236:2C69..2E18`. Roster (`m17.td2`):

- transports: slots 2 and 3, class 1, armour 160 each (setup × 5), from (92, 26) / (94, 26) to (1, 163);
- escorts: slots 5 and 6, class 4, aggression 0Fh, also to (1, 163);
- attackers: slots 8 and 9 (class 7), slots 10, 11 and 14 (class 6), slot 12 (class 6 after setup), slot 13 (class 8).

```text
base(87, 28)
A: if phase = 1 and dead(2) and dead(3) and (tileX(2) = 1 or tileX(3) = 1):
       phase := 100; post(M2[14], 42)                                    (a transport got through)
B: if phase = 1 and dead(2, 3): phase := 2; post(M2[15], 42)             (both destroyed)
C: if phase = 2 and ticker idle: trigger := 1                            (2236:2D1B; U-200)
for a = 8..14:
    engage a → player, 15
    for b = 5, 6: engage a → b, 20
    for b = 5, 6: engage b → a, 5
    for b = 2, 3: engage a → b, 25
exit 2 @ (1,163); exit 3 @ (1,163); exit 5 @ (1,163); exit 6 @ (1,163)
```

What the player must do, and what happens:

| Transports at the end | Script | End of flight |
|---|---|---|
| Both reach (1, 163) | A: phase 100, M2[14] | Return to base → succeeded, outcome 1 |
| One reaches it, the other is destroyed | A (A needs both gone and either at X = 1) | Return to base → succeeded, **outcome 0**: debrief "mission failed", retry prompt; a decline decrements the mission (episode-deltas §8.2) |
| Both destroyed | B: phase 2, M2[15]. When the ticker goes idle (155 or 156 ticks later), C sets the trigger, and `Msn_E2FlightEvents` sets `g_MissionLost` := 1 **in the same tick** (§4.7.1) | Flight ends as lost (no cut-scene in E2): "lost" retry prompt; a decline leads to ending C |

### 5.18 Flight 18 (stop the Vulture IIIs before they reach the base)

`2236:2E19..2F65`. Roster (`m18.td2`): friendly slots 2, 3, 4 are class 4; Vulture IIIs 6..11 are class 6. 9..11 are hidden by setup. Every Vulture III path passes through the base tile (92, 31), the player's start tile. Slots 7, 9 and 11 end at (92, 30); slots 6 and 10 at (92, 31).

```text
base(87, 28)
A: if dead(6, 7, 8) and phase = 1:
       for s = 9..11: class(s) := 6
       post(M2[16], 37); phase := 2                                      (colour 37)
allGone := 1;  for s = 6..11: if class(s) ≠ FFh: allGone := 0
B: if allGone and phase < 100: post(M2[17], 42); phase := 100
for s = 6..11: if tile(s) = (92,31): trigger := 1                        (2236:2F15; no class test; U-200)
for a = 2..4: for b = 6..11: engage a → b, 15
```

Notes:

- A runs before the allGone count, so the second wave is live when B counts. B needs all six gone.
- The flight has **no exit rules and no rule against the player**.
- **The trigger has no class or phase test.** Tiles only change in step 10, so "a ship on (92, 31) at step 9" covers every way of reaching the tile, alive or as a wreck.
- Phase 100 does not disarm the trigger. If one Vulture III reached the base and the player then clears the rest and returns to base within 148 ticks, `succeeded` ends the flight before `Msn_E2FlightEvents` sets `lost` (U-247).
- Once the trigger is set, the base-destruction sequence of §4.7.1 runs. The flight is lost 148 ticks later.

### 5.19 Flight 19 (Glasya)

`2236:2F66..3052`. Roster (`m19.td2`):

- slot 2: Glasya, class 8, armour 1000, aggression FFh, patrolling from (175, 175);
- slots 3..7: class 7.

Comm viewer: phase 2 → screen 5, phase 3.

```text
base(87, 28)
A: if phase = 1 and Msn_NearLive(2, player, 45): post(M2[10], 42); comm pending := 1; phase := 2
B: if dead(2): comm pending := 0
C: if dead(2) and phase < 100: post(M2[18], 42); phase := 100
engage 2 → player, 20
for s = 3..7: engage s → player, 30
```

- A has no ticker test. It cuts off whatever is scrolling, including the objective text, and posts only once.
- Destroying Glasya completes the flight with or without the transmission.

### 5.20 Flight 20 (the last convoy leaves to the north)

`2236:3053..31DB`.

Roster (`m20.td2`), all convoy ships ending at the north exit (148, 1):

- slots 2..5: class 4;
- slots 6 and 7: class 2;
- slot 10: class 5;
- slot 17: class 1;
- slots 19 and 20: class 0.

Vultures 12..15 (class 7) start at (103, 40) and (120, 30..34). Guidance home point: straight up from the player (episode-deltas §6).

```text
if phase = 100 and viewY <= 7: succeeded := 1                            (signed; never reset to 0)
for s = 2..10:  exit s @ (148,1)
for s = 17..20: exit s @ (148,1)
for a = 2..7:  for b = 12..15: engage a → b, 10;  engage b → a, 15
for b = 12..15: engage b → 10, 15
for a = 17..20: for b = 12..15: engage b → a, 15
allGone := 1;  for s = 12..15: if class(s) ≠ FFh: allGone := 0
if allGone and phase < 100: post(M2[19], 42); phase := 100
```

Notes:

- **No base test.** After the four Vultures are gone the player must fly to the top of the map: view Y ≤ 7, and 7 is the lowest view Y.
- No rule engages the player, and the convoy's fate is not tested.
- `Sim_FlyMission` keeps E1's flight-20 rule at `0E96:EA00..EA17`: at the end of every tick (before the end-of-flight test), if viewX > 170 and viewY > 170 (signed), player armour := 64.
- Exits for the empty slots 8, 9, 11 and 18 are harmless.

---

## 6. Edge cases and original bugs

| Quirk | Original behaviour | Port policy |
|---|---|---|
| Kill seen one tick late | A ship killed at step 6 stays live for the rules of step 8 in the same tick (episode-deltas §5.1 P3) | Preserve (T1) |
| Escaped targets count as done | Exit rules and the AI exit marker count like destruction in flights 4, 5, 8; flights 3 and 9 separate the cases only in the outcome | Preserve |
| Base tests without phase | Flight 6 ends when the player reaches (118..123, 70); flight 7 ends at (102, 104) whether or not the transmission was read | Preserve |
| Base tests differ in the sub-tile limit | subY ≤ 36 (flights 1–3), ≤ 35 (7), ≤ 31 (9), none (4, 5, 6, 8) | Preserve |
| Flight 2 slot 8 | Setup's class loop 4..8 gives the empty slot 8 class 3 at tile (0, 0); the map-edge rule removes it in tick 1 | Preserve; harmless (U-243) |
| Flight 3 arrival before the path end | The truck is hidden at x 85..87 and its tile moved to (84, 135) | Preserve (outcome depends on it) |
| Flight 4 slot 9 exit | Its exit rule tile is not on its (looping) path | Preserve; inert |
| Flight 8 slot 11 exit tile | Rule tests (1, 122), path ends at (1, 123); slot 11 loops and must be destroyed for phase 3 to end | Preserve in Classic; **fix-enhanced** candidate (OB-036, U-242) |
| Flight 8 never targets the player | No engagement rule names the player | Preserve |
| Flight 5 stale indicator | Comm pending stays 1 if phase 100 comes before F6; F6 then does nothing | Preserve (OB-037) |
| Flight 7 one-tick indicator | Slot 2 dead: B sets the flag after A cleared it; indicator and sound for one tick | Preserve (OB-037) |
| Comm viewer ignores comm pending | F6 in the right phase shows the transmission even with the indicator off | Preserve |
| Ticker gates | The M key and the jurisdiction warning restart an idle ticker and delay the announcements of flights 1, 5, 7 | Preserve |
| Base test during the end sequence | The rules keep running after the player is destroyed; a base condition that becomes true before end tick 35 ends the flight as succeeded with `g_PlayerDied` = 0 (U-244) | Preserve |
| Shared loop counters | DS:31E6/31E8 (and DS:197A in the outcome test) are globals | Harmless; a port may use locals |

New rows for [original-bugs](../original-bugs.md) (proposed; RE-9 integrates):

| ID | Bug | Where | Impact | Policy |
|---|---|---|---|---|
| OB-036 | E2 flight 8: the exit rule of slot 11 (last convoy's escort) tests tile (1, 122) but its path ends at (1, 123). It never leaves, its path loops back to the hangar, and phase 3 (and so the flight) waits until it is destroyed by an enemy or by the player. | §5.8; `td2:2236:1C47..1C50`; `m8.td2` slot 11 path | Prolongs the flight; with no enemy left the player must shoot its own escort | preserve (Classic); fix-enhanced candidate: exit at (1, 123) |
| OB-037 | E2 transmission flag quirks: flight 5 never clears comm pending when the transports are gone before F6 (indicator pulses to the end, F6 does nothing); flight 7 clears the flag before it may set it, so with slot 2 already gone the announcement lights the indicator and plays sound 200 for one tick. | §5.5, §5.7; `td2:2236:177C`, `2236:1922..195E` | Cosmetic | preserve |

### Flights 10–20

| Quirk | Original behaviour | Port policy |
|---|---|---|
| **OB-038** Flight 16 transmission gate | Rule C tests `ticker = 1` (just restarted) instead of idle. The transmission (screen 7) appears only when the ticker has just been restarted (in practice by M) while the player is within 25 tiles of slot 10 in phase 1. It is probably a typo for `= 0` | Preserve in Classic. Fix candidate (test idle) for enhanced only |
| **OB-039** Flight 13 ghost volleys | The script raises fire request (2, heading(2)) every tick, alive or not, and the volley spawner (`0E96:E05D..E23B`) has no class test. After the Areo-1 is destroyed or removed, its requests go on from its last tile and heading. Every second tick a volley is spawned with shot-table bytes read for class FFh: DS:0443 + 17 × 255 + m = DS:1533..1543, inside `g_FlightPalette`. With `td.pal` loaded these 17 bytes are 15, 27, 27, 15, 27, 39, 15, 15, 39, 27, 15, 39, 39, 15, 27, 39, 15. That gives up to 17 shots per volley with kinds 14, 26 or 38, and `Snd_PlayEffect` with id 255. This only happens when the last heading is 2..8 | Preserve pending U-246; record in original-bugs |
| **OB-040** Flight 10 outcome unreachable | The outcome test wants tile(2) = (151, 199), but slot 2 is removed by its exit rules on (149..153, 198) first, so the debrief never reports a flight-10 failure | Preserve |
| OB-025 Flight 16 outcome | Always 1 (§4.8.1) | Preserve (existing) |
| Flight 10 phase 18 | The phase goes 1 → 18 → 2 → 100 | Preserve the values (the comm viewer and saves see them) |
| Flight 11 stuck comm lamp | If slot 2 is gone before F6, the phase jumps to 4 and comm pending stays 1 to the end | Preserve |
| Flight 11 D re-spawns 11 and 12 | No lasting effect (§5.11) | Preserve the order |
| Flight 12 cannot be failed | Destroyed trucks count as delivered; no outcome test | Preserve |
| Flight 13 RNG | One `Random(9)` per tick in step 9, also after slot 2 is gone and after phase 100 | Preserve (T1) |
| Flight 17 partial delivery | One transport through, one destroyed: success message and success at base, but outcome 0 (failure debrief, retry) | Preserve |
| Flight 17 lost delay | Lost is set only when the ticker goes idle after M2[15] (155–156 ticks), or later if M restarts the ticker at the right moment | Preserve |
| Flight 18 trigger | No class or phase test, so it can still fire after phase 100 | Preserve; U-247 for the success/lost race |
| Flight 18 has no exits and no player engagement | — | Preserve |
| Flight 19 announcement | Not gated on the ticker; it overwrites the scrolling objective | Preserve |
| Flight 20 success | Phase 100 and view Y ≤ 7; `succeeded` is never cleared (harmless) | Preserve |
| Scratch flag DS:31C5 | Flights 18 and 20 reuse `g_MsnEventCount` as an "all gone" flag | A port may use a local |

## 7. Test vectors

Static unless the evidence column names a capture. "Tick *t*" means the tick in which the stated cause happens.

| # | Setup | Expected | Evidence |
|---|---|---|---|
| C1 | E2 flight 1, no input, tick 1: slot positions after setup (view (102, 104); slot 3 at (21, 159), 7 at (40, 152), 13 at (30, 98), 1 at (20, 89); [e2-mission-setup §6](e2-mission-setup.md) vector 1) | Of all engagement rules of §5.1 only `engage 3 → 7` (|dx| 19, |dy| 7 < 20) and `engage 13 → 1` (10, 9 < 15) fire; slots 3 and 13 run an attack step in tick 1 (one RNG draw each), phase stays 1 | C-0015 (ticks 1–2; [e2-mission-ships §6](e2-mission-ships.md) vector 3) |
| 1 | `Msn_NearLive(1, 0, 15)`, view (102, 104) (player tile (107, 107)), tile(1) = (121, 110), class(1) = 2 | true (\|107 − 121\| = 14 < 15, \|107 − 110\| = 3) | static |
| 2 | Same with tile(1) = (122, 107) / (92, 107) / (93, 107) | false / false / true | static |
| 3 | Same with class(1) = FFh | false | static |
| 4 | Flight 1 base test: view (102, 104), subY 36, phase 100 / subY 37 / phase 3 | succeeded 1 / 0 / 0 | static |
| 5 | Flight 7 base test: view (102, 104), subY 35, phase 1 / subY 36 | 1 / 0 | static |
| 6 | Flight 9 base test: view (87, 28), subY 16, phase 100 / subY 32 | 1 / 0 | static |
| 7 | Flight 5 base test, phase 100: view (178, 35) / (183, 35) / (177, 35) / (184, 35) / (180, 34) | 1 / 1 / 0 / 0 / 0 | static |
| 8 | Flight 6 base test, phase 1: view (118, 70) / (123, 70) / (117, 70) / (124, 70) / (120, 71) | 1 / 1 / 0 / 0 / 0 | static |
| 9 | Flight 8 base test, phase 100: view (7, any) / (8, any) | 1 / 0 | static |
| 10 | Flight 3: tile(1) = (87, 135), class(1) = 0, phase 1 at step 8 | after the block: class(1) FFh, tile(1) (84, 135), phase 100, ticker 1, `M1[3]` in colour 37, `M1[4]` not posted, succeeded 0 this tick; teardown outcome 1 | static |
| 11 | Flight 3: truck destroyed by a projectile at (60, 140) in step 6 of tick *t* | tick *t* step 8: no change (still live); step 10: class FFh; tick *t* + 1 step 8: `M1[4]`, phase 100; outcome 0 | static |
| 12 | Flight 9: truck reaches tile (177, 169) in step 10 of tick *t*; player never moved (view (87, 28), offsets (32, 16)) | *t* + 1 step 8: exit rule hides slot 5 (phase still 1); *t* + 2 step 8: phase 100, `M1[10]` colour 42; *t* + 3 step 8: succeeded 1, loop ends at step 33 of *t* + 3; outcome 0 | static |
| 13 | Flight 8: class(2) = class(3) = FFh, phase 1 at step 8 | phase 2; class(5) = 0, class(6) = 0, class(7) = 4 at tile (107, 106); `M1[7]` colour 42; B does not fire in the same tick | static |
| 14 | Flight 5, no input | Phase 1 → 2 at step 8 of the first tick after the objective ticker returned to 0 (78 page-0 ticks after setup); comm pending 1; `M1[1]` colour 42; never repeated | static |
| 15 | Flight 7, phase 2, F6 pressed, back page 0 at step 15 | `UI_CommScreen(3)`; F6 key byte 0; comm pending 0; phase 100. Same F6 in phase 1: request consumed, nothing else | static |
| 16 | Flight 1, phase 1, slot 1 live 10 tiles from the player tile, ticker 5 | no announcement until the ticker is 0; then `M1[1]` colour 42, comm pending 1, phase 2 | static |
| 17 | Flight 2, tick 1 | Slot 8 (class 3, tile (0, 0)) live at step 8, FFh after step 10 | static (U-243) |
| 18 | Flight 8 teardown: tileX(2) = 1, all other slots 3..11 ≠ 1 / none of 2..11 at column 1 | outcome 1 / 0 | static |
| 19 | Flight 16 teardown, any tiles | outcome 1 | static |

### Flights 10–20 (static)

| # | Setup | Expected |
|---|---|---|
| 1 | Flights 10–19: view (87, 28), viewSubY 31, phase 100 | succeeded = 1 at step 9 |
| 2 | As 1 with viewSubY 32 (or view (87, 29)) | succeeded = 0 |
| 3 | Flight 10, objective ticker idle at tick t | Phase 18 and M2[0] at t. After slot 2 is gone: phase 2 and slot 14 class 7, 15 and 16 class 0 (live), all in the same tick |
| 4 | Flight 10, slot 2 removed by its exit at (151, 198) | Teardown outcome = 1. Slot 2 destroyed at (120, 120) → outcome = 1 |
| 5 | Flight 11, slot 6 removed at (177, 167); slots 2 and 11 destroyed elsewhere | Outcome = 0 |
| 6 | Flight 13, any tick | Exactly one `Random(9)` call in `Msn_UpdateB`. Its result is stored in DS:39DF (target heading of slot 2) |
| 7 | Flight 16, phase 1, view (130, 140) (player tile (135, 143)), slot 10 live at (148, 150), ticker idle, M pressed at tick t | At t + 1, step 9: M2[10] posted, comm pending 1, phase 2. Without M (ticker stays 0): nothing |
| 8 | Flight 16, slot 2 removed at (177, 168) | Phase 100 with M2[12]; outcome = 1 (OB-025) |
| 9 | Flight 17, both transports destroyed in tick T (back page 0 in T) | Phase 2 and M2[15] at T. Trigger and lost at T + 155 (T + 156 if T is a page-1 tick). The flight ends in that tick; outcome 0 |
| 10 | Flight 17, both transports removed at (1, 163) | Phase 100, M2[14]; after the base test, outcome = 1 |
| 11 | Flight 18, slot 6 on (92, 31) at step 9 of tick T | n = 2 after T (two explosions); ticker text at T + 3; lost, white DAC and palette copy at T + 148 |
| 12 | Flight 18, slots 6, 7, 8 gone in phase 1 | In that tick: class(9..11) = 6, M2[16] in ink 37, phase 2; allGone = 0 |
| 13 | Flight 19, Glasya at (137, 31), view (87, 28) (player tile (92, 31)) | \|137 − 92\| = 45, which is not < 45: no announcement. At (136, 31): phase 2 |
| 14 | Flight 20, phase 100, view (100, 7) | succeeded = 1. View (100, 8): not set |

## 8. Open questions

- [ ] U-242: E2 flight 8 slot 11 (OB-036): confirm in a capture that it reaches (1, 123), turns back, and that phase 3 waits for its destruction.
- [ ] U-243: E2 flight 2 slot 8: confirm that the class-3 ship that setup creates at tile (0, 0) is removed in tick 1 by the map-edge rule and is never drawn, plotted or counted.
- [ ] U-244: Base test during the end sequence (all episodes): can a flight end as succeeded with `g_PlayerDied` = 0 after the player was destroyed, when the base condition becomes true before end tick 35 (most likely in E2 flights 6 and 7, which have no phase condition)? Needs a capture and a decision for the flow spec.
- [ ] U-204 (existing): the one-tick delay of kills (§4.1) follows from P3; confirm with the first E2 capture (flight 1 or 9 is the simplest).
- [ ] U-205 (existing): first waypoint of ships spawned by flight 8.

### Flights 10–20

- [ ] **U-245:** Flight 16 transmission (OB-038). Confirm in a capture that screen 7 can only be reached by restarting the ticker (M) near slot 10 in phase 1, and that the flight is otherwise unaffected.
- [ ] **U-246:** Flight 13 ghost volleys (OB-039).
  - What do class-FFh volleys do: projectile kinds 14, 26 and 38 (drawing, movement, damage 2 × (kind + 1) to the player under weapons P2) and `Snd_PlayEffect` id 255?
  - Is `g_FlightPalette` really the unmodified `td.pal` at that point?
  - Needs a capture of flight 13 after the Areo-1 is destroyed, plus the weapons and sfx owners.
- [ ] **U-247:** Flight 18 race. If the player completes the flight (phase 100, at base) in the 148 ticks between the trigger and `lost`, the flight ends as a success. Confirm, and check which branch the campaign takes (flow owner). It needs one Vulture III to reach (92, 31) and die there or later.
- [ ] **U-248:** Flight 11: slot 12 (spawned as class 7 by rule C) has its exit at (177, 189), which is not on its path (the path ends at (177, 169)). What does the ship do after its last path step? Does it stay live for the rest of the flight? Its class is re-set by rule D, and it is unmarked, so it is probably harmless. Ask the mission-ships owner (path-end behaviour) and check a capture.

U-205 (first waypoint and RNG order without the setup `AI_ChooseWaypoint` loop) affects every flight in this part.

## 9. Provenance

| Claim | Address / evidence | Method | Confidence |
|---|---|---|---|
| Flight blocks 1–9 (dispatch, base tests, rules, order) | `2236:0AE9..1128` (1), `1129..137C` (2), `137D..156C` (3), `156D..170E` (4), `170F..1845` (5), `1846..18F6` (6), `18F7..1A50` (7), `1A51..1C52` (8), `1C53..1D25` (9); epilogue `1D26..1D29` | static: full listing (checked gap-free), every call's arguments read from the push sequence; decompiler only for orientation | LIKELY |
| Primitives | `2236:05DC`, `2236:0616..070E`, `2236:030A`; `2F63:3232` | static (listing) | CONFIRMED (code) |
| Message runs and counts | `2236:070F..0ADA` (11), `2236:1D2A..2409` (20) | byte scan of `td2.exe` (file offset = 16768 + seg × 16 + off) | CONFIRMED |
| Objectives per flight | `0E96:59D7` (`e2.objectives`), pushes in `0E96:6184..6699` | static | CONFIRMED (code) |
| Start state used by the rules | `Msn_Setup` `0E96:6105..66E7`; slot tiles from path step 1 `0E96:B451..B494`; start views `0E96:B67F..B69B`, `BBE9..BC3B`; mission files `m1.td2`..`m9.td2` (classes, paths) | static + file decode | CONFIRMED (code); setup spec authoritative |
| Tick placement | `0E96:C653..C662` (A/B), `CDAF..CDC2` (comm viewer), `D2D4` (E2 flight events), `D7A3..D7B1` (F6), `D684..D693` (edge hit), `E447..E455` (M key), `EA1C..EA38` (loop exit) | static (listing) | CONFIRMED (code) |
| Comm viewer table | `0E96:83F9..84EB` | static (listing) | CONFIRMED (code) |
| Home points | `0E96:858B..85E3` | static (listing) | CONFIRMED (code) |
| Outcome tests | `0E96:EA54..EB4B` | static (listing) | CONFIRMED (code) |
| `Msn_E2FlightEvents` acts only in flights 17, 18 | `0E96:99BB`, `9A66`, `9BAB` | static (listing) | CONFIRMED (code) |
| Removal rules of `AI_UpdateShips` used in the notes | [mission-ships](mission-ships.md) §4.2, §4.3.5; unchanged in E2 apart from the per-flight cases ([episode-deltas](../../re/episode-deltas.md) §5.2) | spec | LIKELY |

### Flights 10–20 and the event routine

| Claim | Address / evidence | Method | Confidence |
|---|---|---|---|
| Flight blocks 10–20 | `td2:2236:2414..25EC` (10), `25ED..27E7` (11), `27E8..28A6` (12), `28A7..2952` (13), `2953..2A51` (14), `2A52..2B4A` (15), `2B4B..2C68` (16), `2C69..2E18` (17), `2E19..2F65` (18), `2F66..3052` (19), `3053..31DB` (20) | Static. Full listing read with a private call/argument condenser. All 107 calls and every store accounted for. Loop bounds read from the compare at each back edge | CONFIRMED (code); behaviour LIKELY |
| M2 indices | Pushed string offsets mapped onto the 20-element run `2236:1D2A..2409` | Static + byte scan | CONFIRMED |
| Start state | `0E96:66E8..6D51`; ship-record field offsets checked by address arithmetic | Static | CONFIRMED (code) |
| Rosters, paths | `m10.td2`..`m20.td2`, class bytes 40..59, paths (stride 501, structures §2) | Asset decode | CONFIRMED (bytes) |
| Comm viewer rows | `0E96:83F9..84EB` | Static | CONFIRMED (code) |
| `Msn_E2FlightEvents` | `0E96:99B8..9BBF`; resets `0E96:A774`, `A7D4`; call `0E96:D2D4` | Static | CONFIRMED (code) |
| U-200 setters | `2236:2D1B`, `2236:2F15` | Static | CONFIRMED |
| Outcome tests | `0E96:EA54..EB4B` | Static | CONFIRMED (code) |
| Flight-20 armour rule | `0E96:EA00..EA17` | Static | CONFIRMED (code) |
| OB-039 spawner and palette bytes | `0E96:E05D..E23B` (no class test); `td.pal` bytes 559..575 | Static + asset | LIKELY (the effect of the shots is U-246) |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-9b and RE-9c (Archaeologist + Spec author, for RE-9); merged by RE-9 | Initial: E2 architecture, primitives, base tests, comm viewer, outcome tests, flights 1–9 (RE-9b); flights 10–20, `Msn_E2FlightEvents`, U-200 (RE-9c, §4.7, §5.10–§5.20); capture vector C1 (RE-9, C-0015) |
