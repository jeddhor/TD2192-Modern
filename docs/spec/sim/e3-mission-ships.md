# Mission ships and player flight model in Episode 3: first tick and per-tick deltas

| | |
|---|---|
| **Spec ID** | `sim/e3-mission-ships` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (positions, waypoints, AI state, RNG use, player armour and shields per tick), as [mission-ships](mission-ships.md) and [flight-model](flight-model.md) |
| **Confidence** | CONFIRMED: static (byte comparison of `td3:2935:0011` with `td2:2D26:0011`; listing of the E3 hazard test `td3:0C5D:D27A..D3CB` and of the setup `0C5D:A9CC..BB1B`) and two captures: C-0036 (E3 flight 1, `td3.map`, ticks 0–58, no input) and C-0041 (E3 flight 8, `td4.map`, ticks 0–36 without input, then a keypad route into a hazard cell and the whole end sequence) |
| **Tasks** | M10-02 (feeds M10-03 and every E3 mission task) |
| **Unknowns** | U-205 (E2 resolution confirmed for E3), U-201 (as E2), U-273 (resolved, §4.3), U-286 (new) |

Read this as a **delta against [e2-mission-ships](e2-mission-ships.md) rev 1**, which is itself a delta against [mission-ships](mission-ships.md) rev 2 (E1). §5 is the E3 delta section to [flight-model](flight-model.md) rev 2 (the player); `flight-model.md` itself is not changed. Addresses are `td3:<seg>:<off>` (ADG §7.8); DGROUP is `32C5`, and the E3 ship record *s* is at DS:4356 + 429h × (*s* − 1) (E2 address − 4).

## 1. Summary

**Mission ships behave exactly as in E2.** `AI_UpdateShips` (`td3:2935:0011`) is E2's routine (`td2:2D26:0011`): same length (9,138 bytes), and every differing byte is a relocated far-call segment or a DGROUP operand 4 lower ([e3-mission-setup](e3-mission-setup.md) §4.1). The AI unit `2B72` is E1's `34E6`, code-identical ([episode-deltas](../../re/episode-deltas.md) §3.1). The E3 setup, like E2's, has **no `AI_ChooseWaypoint` loop** before the first tick, so every ship's first AI call is the null path step in tick 1 and path-following ships start moving one tick later than in E1 (U-205, [e2-mission-ships §4.3](e2-mission-ships.md)). Both E3 captures confirm this.

**The player's flight model is E1's with five E3 changes** (§5): the hazard tiles 41h–46h (shields to 0 and armour −17 every tick on one, in every flight except 14, 17 and 19), no new afterburner burst in flight 16 from phase 2, the joystick read whenever a stick is present, map collision always active (late draw is never set), and the flight-16 class-table patch. C-0041 confirms the hazard test and shows that its damage continues through the whole end sequence with the signed-byte armour wrapping round (OB-054).

## 2. Definitions and units

As [e2-mission-ships §2](e2-mission-ships.md) and [flight-model §2](flight-model.md), with E3 addresses:

| Term | E3 address | Notes |
|---|---|---|
| Ship record *s* | DS:4356 + 429h × (*s* − 1) | +000h alive flag (§4.2), +001h/+002h waypoint, +003h/+004h previous, +00Bh path cursor (u16), +3F6h aggression, +3F8h shields, +3FAh armour, +415h/+41Bh/+41Fh attack order |
| Class, tile | DS:399D + *s*, DS:3A51 + *s* / DS:3A65 + *s* | |
| Tick count | `g_MissionTicks` DS:1638 (u32) | "tick *t*" = the pass in which it becomes *t*; tick 0 = the last setup state |
| Player armour, shields | `g_PlayerArmour` DS:38B2, `g_PlayerShields` DS:38B3 | i8 |
| View | `g_ViewTileX/Y` DS:395A/395C, `g_ViewSubX/Y` DS:395E/3960 | i16; offsets 16..48 |
| Map | far pointer `g_Map` DS:4352 | 201 × 201, cell (x, y) at x × 201 + y |
| `RandSeed` | DS:064C (u32) | |
| Mission over, end ticks, died | DS:31DB, DS:31DC, DS:11ED | |

## 3. Data

No new data. Hazard codes: map tiles **41h..46h** (attribute 2, open; [e3-mission-setup §3.5](e3-mission-setup.md)). Only `td4.map` contains them (37 cells: (40, 121..123), (40, 131..133), (46..48, 109), (46..48, 118), (53..55, 114), (53..55, 118), (53..55, 137), (62..64, 114), (66, 121..123), (66, 128..130), (74, 77..79), (91, 87..90)); flight 12 replaces 27 of them with tile 26h until tick 6480 ([e3-mission-logic §4.6.3](e3-mission-logic.md)).

## 4. Behaviour: mission ships

### 4.1 Setup

As [e2-mission-ships §4.1](e2-mission-ships.md): `AI_ResetShipRecords` with its 20 `Random(2)` draws, the mission file `m<n>.td3` (every flown flight has one), shields, armour, path copy, waypoint = return = path step 1, tile = path step 1, headings 6, offsets (16, 16), speeds 0, `Msn_Setup`. **There is no step 6** (no `AI_ChooseWaypoint` loop): the setup ends with the DOS-time sample and falls into the tick top `0C5D:BB20`. Neither the setup nor `Msn_Setup` calls `Random` ([e3-mission-setup §4.1](e3-mission-setup.md)). After setup, for every slot: path cursor 0, previous (0, 0), move direction 0, waypoint = tile = path step 1, +000h = 0.

### 4.2 Per tick (`AI_UpdateShips(flight)`, `td3:2935:0011`)

Identical to E2 ([e2-mission-ships §4.2](e2-mission-ships.md)): the prologue writes record +000h := 1 for live slots and 0 for empty ones (no reader, U-201); no per-flight cases (the flight argument is ignored); no flight-19 block after the call in `Sim_FlyMission`.

### 4.3 What the first AI call does (U-205, confirmed for E3)

[e2-mission-ships §4.3](e2-mission-ships.md) applies unchanged, with these E3 points:

1. **Ships released later by a timer or a rule** behave like E2's later spawns: their first AI call is the null path step (cursor 0 → 1), and they start toward path step 2 one tick later. In E3 this concerns flight 1 (slots 11..14 in phase 3, slots 2..4 in phase 5), flight 10 (slot 4 at the end of tick 10, slot 5 at tick 301) and flight 14 (slots 6..11 at the end of tick 25) ([e3-mission-logic](e3-mission-logic.md) §5.1, §5.10, §5.14).
2. **No empty slot gets a class at setup in E3** (no counterpart of E2's OB-035): every class override of `Msn_Setup` hits a slot that has a ship in the mission file, or hides one ([e3-mission-setup §3.2](e3-mission-setup.md)).
3. **Ships that start on row 1 or column 1 are not removed** in their first ticks (U-273 resolved): in C-0036 the class-8 ships of flight 1 start on (80, 1) and (78, 1), take the null step in tick 1, step to (80, 2) and (78, 2) in tick 2 and are on row 28 at tick 58, still live. The map-edge rule ([mission-ships §4.2.3](mission-ships.md)) does not fire for row or column 1.
4. **RNG.** The setup draws nothing after `AI_ResetShipRecords`. In both captures `RandSeed` is 05322D58h at the top of tick 1 (the same value as E2's C-0015/C-0016: the program is started directly with the oracle's fixed clock) and stays unchanged as long as no ship gets an attack order (C-0036: ticks 1–58; C-0041: ticks 1–296, until the retry prompt's `Random(10)`).

## 5. Player flight model: E3 delta to [flight-model](flight-model.md) rev 2

Everything in flight-model.md applies to E3 except the following ([episode-deltas §4.2](../../re/episode-deltas.md#42-flight-model), confirmed and completed here).

### 5.1 Hazard tiles (new; `td3:0C5D:D27A..D3CB`)

**Where in the tick.** In the step-27/28 block of [tick-order](tick-order.md), after the drift and drag of the four speed components and **before** map collision ([flight-model §4.3](flight-model.md)) and the tile carry (§4.4). The view and offsets used are therefore those left by the previous tick's carry (or by this tick's drift, for the offsets). It runs every tick, also while the mission is over (end sequence), and on both draw pages.

```text
if flight is 14, 17 or 19: skip the whole test
hazard(dx, dy) := 41h <= map[viewX + dx][viewY + dy] <= 46h        (cell index (viewX+dx)*201 + (viewY+dy), 16-bit)
hit := hazard(5, 2)
       or (hazard(4, 2) and offsetX <= 32)
       or (hazard(5, 2) and offsetY <= 32)          (repeats the first test; never decides anything)
       or hazard(4, 3)
if hit:
    Snd_PlayEffect(9, FFh, FFh, low byte viewX, low byte viewY, sfx device)   (the player's own sound: distance 0)
    player shields := 0
    player armour := (armour - 17) as a signed byte (8-bit wrap)
```

- The tests are made in the order written and stop at the first hit; only one hit per tick.
- **Damage per tick spent on a hazard:** shields 0 and armour − 17. There is no immunity time: a ship that stays on (or keeps touching) a hazard cell loses 17 every tick. Class 6 (armour 31) is destroyed after two ticks, class 2 (15) after one.
- **Destruction** follows the normal rule ([flight-model](flight-model.md), [tick-order](tick-order.md) step 26): armour < 1 is seen at step 26 of the **next** tick, which starts the end sequence (mission over, end ticks counting to 35).
- **The test does not look at the mission-over flag.** During the end sequence the player keeps losing 17 per tick if the view still touches a hazard; the armour byte wraps from −122 to +117 and goes on falling (OB-054). It has no effect on the result: the loop ends at end tick 35 and `g_PlayerDied` comes from the end-tick count.
- **Sound.** Effect 9 (`punyexp.voc`, priority 0) at full volume with a Sound Blaster ([sfx-policy](../audio/sfx-policy.md) §4.1: source FFh = distance 0, the player's own sound). It is requested every tick of contact; with the PC speaker (device 3) the effect path may call `Random` ([sfx-policy](../audio/sfx-policy.md) §3.1 and §4.5: per effect id), which then shifts the shared RNG.
- **Mission ships** are not affected: `AI_UpdateShips` has no hazard test (E2 code), and the tiles are open to them.
- **Flights.** The test is skipped in flights 14, 17 and 19, which start at view (88, 76), between the hazard cells (74, 77..79) and (91, 87..90). It applies in every other flight, but only `td4.map` (flights 7–19, 21) has hazard cells; in flight 12 most of them are off until tick 6480.

### 5.2 Other E3 changes to the player

| # | Change | Address | Effect |
|---|---|---|---|
| 1 | Flight 16, phase ≥ 2: no new afterburner burst | `0C5D:BC37..BC43` | The burst key press is consumed; a burst already running continues ([e3-mission-logic §4.7.1](e3-mission-logic.md)) |
| 2 | Joystick read whenever a stick was detected, without the `g_CfgController = 2` test | tick-order step 17 | A connected stick steers even when the keyboard is configured ([episode-deltas §4.1](../../re/episode-deltas.md) step 17) |
| 3 | Late draw is never set | — | Map collision ([flight-model §4.3](flight-model.md)) runs in every E3 flight |
| 4 | Flight-16 class-table patch (phase 3 → 5) | `0C5D:9907..9946` | Classes 0..8: max speed 0, accel 0; class 2: 20 / 4. The player (class 2 in flight 16) keeps flying at top speed 20 (biased; 4 px per tick) ([e3-mission-logic §4.6.4](e3-mission-logic.md)); restored by flight 17's setup |
| 5 | Infra-red unreachable | `0C5D:E486..E607` | Palette mode is always 0 in E3, so the Alt handler, which acts only in modes 1 and 2, never runs |
| 6 | No flight-20 "view beyond (170, 170) → armour 64" rule | — | E2 kept it; E3 has no flight 20 |

## 6. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| **OB-054**: hazard damage during the end sequence | The hazard test has no "mission over" condition, so a destroyed player who still touches a hazard loses 17 armour every tick until end tick 35; the i8 armour wraps round (C-0041: 14, −3, −20, …, −122, then 117, 100, …). The HUD shows whatever the armour byte holds (U-286) | preserve (Classic); no effect on the outcome |
| Redundant third clause | `hazard(5, 2) and offsetY ≤ 32` repeats the first test | preserve (no effect) |
| Hazard tested before collision and carry | A move that carries the view onto a hazard is only damaged in the next tick (C-0041: carry to view X 35 in tick 260, first damage in tick 261) | preserve (T1) |
| One-tick delay of path starts | §4.3 (as E2) | preserve |
| +000h written every tick, never read | §4.2 (as E2) | may be omitted |
| OB-013..OB-017 | AI-unit quirks of E1 | carry over unchanged |

## 7. Test vectors (captures)

| # | Capture | Check | Result |
|---|---|---|---|
| 1 | C-0036, E3 flight 1, tick 0 (last setup frame 693) | live slots 5, 6, 8, 9: cursor 0, previous (0, 0), waypoint = tile = path step 1, +000h = 0; hidden slots 2..4, 11..14 have their path step 1 as tile and waypoint | as stated |
| 2 | C-0036, tick 1 (frame 695) | slots 5, 6, 8, 9: cursor 1, previous = waypoint = tile, +000h = 1; hidden slots unchanged (cursor 0, +000h = 0); no attack order written; `RandSeed` 05322D58h | as stated |
| 3 | C-0036, tick 2 (frame 699) | cursor 2; waypoints one step toward path step 2: (85, 160) → (85, 159), (85, 162) → (85, 161), (80, 1) → (80, 2), (78, 1) → (78, 2) | as stated |
| 4 | C-0036, ticks 3–58 | no class change, no attack order, `RandSeed` unchanged; slots 8, 9 reach row 28 (U-273) | as stated |
| 5 | C-0041, E3 flight 8 (`td4.map`), ticks 0–2 | same pattern for all 10 live slots 1..10 (tick 1: cursor 1, waypoint unchanged; tick 2: first step, for example slot 1 (22, 166) → (22, 167), slot 9 (95, 96) → (94, 96)) | as stated |
| 6 | C-0041, keypad route (kp2, kp6, kp8, kp6 at frames 1144, 1202, 1458, 1981 relative to the `td4.map` open at frame 1004; flight-model rules) | tick 260: view carried to (35, 130), offsets (23, 29); no damage in tick 260 (the test ran before the carry) | as stated |
| 7 | C-0041, tick 261 | view (35, 130), offsets (31, 29): `hazard(5, 2)` = cell (40, 132) = 45h → armour 31 → 14, shields 31 → 0 | as stated |
| 8 | C-0041, tick 262 | still on the cell: armour −3; mission over and end tick 1 in tick 263 (step 26 of the next tick) | as stated |
| 9 | C-0041, ticks 263–296 | armour −20, −37, …, −122, 117, 100, …, −86 (−17 every tick, 8-bit wrap); end ticks reach 35 in tick 296; the loop ends; outcome 0 (flight-8 truck test); `RandSeed` unchanged until the retry prompt | as stated |

## 8. Open questions

- [ ] **U-286** (new): what the HUD armour gauge and lamps show while the end-sequence armour wraps round (OB-054), and whether effect 9 is heard every tick of contact on a Sound Blaster (priority 0 re-triggers). Presentation only; check the framebuffers of C-0041 ticks 263–296 and a WAV capture. P3.
- [ ] Long-run E3 ship behaviour has not been compared with a model beyond tick 2 (the E1 model of [mission-ships §6](mission-ships.md) should apply with §4).

## 9. Provenance

| Claim | Address / evidence | Method | Confidence |
|---|---|---|---|
| `AI_UpdateShips` = E2 | `td3:2935:0011..23C2` vs `td2:2D26:0011..23C2` | raw byte comparison, every difference classified (RE-10d) | CONFIRMED |
| No setup `AI_ChooseWaypoint` loop, no setup `Random` | `td3:0C5D:A9CC..BB1B` complete call list | static + C-0036, C-0041 | CONFIRMED |
| First-tick behaviour, late spawns, U-273 | §4.3 | C-0036 (ticks 0–58), C-0041 (ticks 0–2) | CONFIRMED (captured flights); LIKELY (timer spawns) |
| Hazard test: order, cells, exclusions, damage, sound | `td3:0C5D:D27A..D3CB` | static (listing) | CONFIRMED |
| Hazard timing, end-sequence damage, armour wrap | — | C-0041 (frames 1940–2200, every frame) | CONFIRMED |
| Other player changes (§5.2) | `0C5D:BC37..BC43`, `9907..9946`, `E486..E607`; episode-deltas §4.1, §4.2 | static | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-05 | RE-10 (Archaeologist + Spec author + Verifier, M10-02) | Initial: E3 ships = E2 (AI routine identity, no setup waypoint loop, first tick), U-273 resolved; player flight-model delta (hazard tiles, flight-16 rules, joystick, collision, infra-red); OB-054, U-286; vectors from C-0036 and C-0041 |
