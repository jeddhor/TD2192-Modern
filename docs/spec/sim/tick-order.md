# In-flight tick order (`Sim_FlyMission`)

| | |
|---|---|
| **Spec ID** | `sim/tick-order` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (the order of state updates inside a tick decides collisions, kills and RNG consumption) |
| **Confidence** | LIKELY for the order and the addresses (static, read from the instruction stream). Sub-step names are provisional; their internals are the subject of M6–M8 specs |
| **Tasks** | M1-04, M6-01 (resolves U-033) |
| **Unknowns** | U-033 (resolved), U-041, U-044 |

## 1. Summary

`Sim_FlyMission(n)` (`file0001:0E8B:B880`) runs one flight: setup (allocate buffers, load the map, tiles, sprites, palette and mission file `m<n>.td`, apply per-mission overrides, hook INT 09h), then a loop of fixed **ticks** until the mission ends, then teardown. One tick = one pass of the loop from `0E8B:CC7F` back to itself. Inside a tick the original **draws the world into the back page first** (tiles, own muzzle flash, projectiles, player ship, mission ships, explosions, palette effects), then runs the HUD routines, **waits for the tick boundary**, flips the page, and only then handles **player input and player physics** (shields, movement, map collision, thrust, steering, special weapons, cheats). The next tick's drawing therefore shows the player position computed at the end of the previous tick. Mission ships (AI) are moved and drawn by one routine (`3292:0011`) between the player-ship draw and the explosions.

## 2. Definitions

| Term | Meaning |
|---|---|
| Tick | One loop pass; paced by `Sim_TickLimiter` ([timing](../core/timing.md)) |
| Back page | `g_DrawPage` DS:3A30 (0 or 1), toggled once per tick after the flip |
| Mission over | `g_MissionOver` DS:32B7 = 1 (player destroyed, mission finished, or flight 21 timer). From then on `g_EndTicks` DS:32B8 counts up in `0E8B:56ED`; the player is no longer drawn once it reaches 15 and the loop ends at 35 |
| Late player draw | `g_PlayerDrawLate` DS:1703: when non-zero the player ship is drawn after the mission ships instead of before them (set by per-mission setup `0E8B:60D8` and by `0E8B:0984`) |

## 3. Data

Routines called once per tick, in call order (all `file0001:`):

| # | Address | Provisional name | Condition |
|---:|---|---|---|
| 1 | inline `0E8B:CC7F` | tick counter | always |
| 2 | inline `0E8B:CCBE..CCDD` | clear per-tick flags DS:3BE2..3BEA | always |
| 3 | inline `0E8B:CCE3..CD22` | `Sim_PlayerTurnStep` | target heading ≠ heading |
| 4 | `386E:191C` × 66 | `Gfx_DrawTile` (11 × 6 map window) | always |
| 5 | inline `0E8B:CDA0..D160` + `1E2A:006A` | `Sim_AfterburnerBurst` (flame sprites, speed kick, sound) | Ins / keypad + / joystick button 1, player class 2 or 5 |
| 6 | `0E8B:0984` | `Sim_ProjectilesAndHits` ([weapons §4.3](weapons.md)) | always |
| 7 | `386E:024A`…`386E:172D` (one of 16) | `Gfx_DrawPlayerShip` | not late, end ticks < 15 |
| 8 | `2404:1426` | `Msn_UpdateA` (mission triggers and messages) | always |
| 9 | `2404:3521` | `Msn_UpdateB` (mission triggers and messages) | always |
| 10 | `3292:0011(n)` | `AI_UpdateShips` (move and draw the 20 mission ships; [mission-ships](mission-ships.md)) | always |
| 11 | `0E8B:56ED` | `Sim_EndSequenceStep` | mission over |
| 12 | inline `0E8B:D68C..D86F` + `386E:191C` | `Gfx_RedrawOverlayTiles` (tiles drawn over ships) | not late |
| 13 | inline `0E8B:D872..D93x` + `386E:024A` | `Gfx_DrawExplosions` (30 slots) | always |
| 14 | inline + `386E:1901` | `Gfx_PaletteEffects` (hit flash, IR modes, white flash) | flash active |
| 15 | `0E8B:80D3` | `UI_CommViewer` (F6) | comm requested and back page = 0 |
| 16 | (step 7 code) | `Gfx_DrawPlayerShip` | late, end ticks < 15 |
| 17 | `1EBF:0026` | `Inp_JoystickRead` | joystick present and controller = joystick |
| 18 | `0E8B:9860` | `Msn_UpdateEvents` (respawns, scripted events; [mission-logic](mission-logic.md) §4.7; earlier `UI_UpdateStatusPanel`) | always |
| 19 | `0E8B:8E87` | `UI_DrawInstruments` (gauges, radar, guidance line, jurisdiction warning) | always |
| 20 | `0E8B:E05C` | `Sim_TickLimiter` | always |
| 21 | `0E8B:4947` | `Txt_DrawFlightText` (IR-mode notice) | IR mode and tick counter ≤ 200 |
| 22 | `386E:1A6C` | `Gfx_FlipPage` | always (twice in source, one per page) |
| 23 | inline `0E8B:E1D8..` | pause (P key): wait for any key | P held |
| 24 | `0E8B:794C` | `UI_FlightMenu` (Esc) | Esc held and not over |
| 25 | `2C37:03D3` | WORX "sound busy" poll | SFX = Sound Blaster |
| 26 | inline `0E8B:E540..E5A0` | armour/shield checks, shield regeneration | always |
| 27 | inline `0E8B:E5A0..EA2B` | `Sim_PlayerMove` (drag, scroll, map collision, tile carry, map edges) | always |
| 28 | `0E8B:A714` | ship-switch screen (F5) | F5 held, mission ≠ 14 |
| 29 | inline `0E8B:EA2B..F3xx` | `Sim_PlayerThrust` and steering | always |
| 30 | inline `0E8B:F3xx..F6A0` + `1E2A:006A` | `AI_FireRequests` (spawn mission-ship shots) | back page ≠ 0 |
| 31 | `0E8B:B1EC` | `Sim_PlayerSpecials` (Space fire, Alt infra-red, M ticker; [weapons §4.1](weapons.md)) | always |
| 32 | inline `0E8B:F6B5..F7E0` (+ `3A7E:009D`) | cheat key chords | chord held |
| 33 | inline `0E8B:F7E0..F842` | end-of-flight test | always |

## 4. Behaviour

```text
setup (0E8B:B880..CC7E), once per flight:
  stop music; reset mission kills, abort and loss flags; record the heap mark
  allocate the heap buffers; load td.map, td.pal, td.sc3, td.lt2, td.sc2, td.fac, td.shp, td.exp,
    td.wea, td.shr, and mission-specific sprites (mine.td for 7, lander.td for 20,
    td.hel + td.h2 for 12, 15 and 21)
  unless flight 21: load m<n>.td (4 header arrays + both path arrays; structures.md section 2)
  initialise the 20 ship records from the class tables and paths; per-mission setup 0E8B:60D8
    (player class, start position, slot overrides); init each slot (34E6:3449 for slots 1..20)
  clear the key table, hook INT 09h (0E8B:0653), fade in, draw the HUD frame (td.win)
  choose the flight song (mission mod 3); explosions idle; armour and shields from the class tables
  g_TickPrevTime := DOS time

each tick (0E8B:CC7F):
   1. g_MissionTicks += 1 (u32). Flight 21: when it becomes 25, mission over := 1, end ticks := 1
   2. clear the 9 per-tick bytes DS:3BE2..3BEA
   3. player turn step (flight-model.md 4.6)
   4. draw the 11 x 6 tile window: tile (viewX + i, viewY + j), i = 0..10, j = 0..5, into the back page
   5. afterburner burst of player classes 2 and 5 (flight-model.md 4.7)
   6. projectiles, hits and kills (0E8B:0984)
   7. if not late and end ticks < 15: draw the player ship sprite for the current heading
   8. mission logic part A (2404:1426)
   9. mission logic part B (2404:3521)
  10. mission ships: move and draw (3292:0011, argument = flight number)
      flight 19 with mission phase > 1: cap player shields at 10 and pin four slot values
  11. if mission over: end-sequence step (0E8B:56ED; counts g_EndTicks)
  12. if not late: redraw the overlay tiles (codes 42h, 44h, 45h, 46h, and the special cells
      x = 7Ah..7Fh at y = 47h) found in window columns 3..6, rows 1..4, over the ships
  13. explosions: for slots 1..30 with frame < 8: draw sprite 13h + frame if on screen, then frame += 1
      (frame 7 is skipped: 6 -> 8)
  14. palette effects: hit flash (DS:12C4 decays by 2, or 3 in palette mode 1), flash frames (DS:1828)
  15. if the comm viewer was requested (F6) and the back page is 0: run it
  16. if late and end ticks < 15: draw the player ship now
  17. read the joystick when enabled
  18. mission events: respawns and scripted events (0E8B:9860, mission-logic 4.7)
  19. instruments (0E8B:8E87)
  20. wait for the tick boundary (Sim_TickLimiter); g_TickPrevTime := now
  21. palette mode 1 (IR) during the first 200 ticks: draw the IR notice
  22. flip to the back page (hardware scroll offset from viewSubX/viewSubY)
  23. pause key: show the pause text, clear all key bytes, wait until any key byte is set
  24. g_DrawPage := 1 - g_DrawPage
  25. Esc and not over: flight menu (may set g_MissionAborted)
  26. SFX on Sound Blaster: when WORX reports idle, clear g_SfxPriority (DS:1F0A)
  27. shields < 1 -> 0; armour < 1 and not over -> mission over (player destroyed)
      shields < class maximum -> shields += 1
  28. player drift and drag, map collision, tile carry and map limits (flight-model.md 4.2-4.4)
  29. flush the BIOS keyboard buffer; F6 -> comm request; F5 -> ship-switch screen (not in mission 14)
  30. thrust and steering from the arrow keys or joystick (flight-model.md 4.5-4.6)
  31. (details: mission-ships.md section 4.4.2; the shot heading is the ship's heading, not the request entry)
      every second tick (back page currently 1): for each slot 1..20 and each request entry 2..8
      that is set (g_FireRequest, DS:3BEC + 9*slot + entry), and for each muzzle m = 1..17 whose
      entry in the class's shot table (DS:02E5 + 17*class + m) is non-zero: take the highest-numbered
      free projectile slot (0..40), spawn a shot at the ship's tile and sub-pixel position plus the
      muzzle offset for its heading (DS:035C + 34*heading + 2m), play one sound
      (structures.md section 4)
  32. clear all fire requests (slots 1..20, entries 1..9)
  33. player specials (0E8B:B1EC): fire (Space), infra-red toggle (Alt), ticker replay (M)
      (the Tab weapon switch is done in step 32's tail, 0E8B:F4F8..F6A7, before 0E8B:B1EC)
  34. cheat chords (globals.md section 5)
  35. if aborted, or end ticks = 35, or mission lost, or mission succeeded: leave the loop
teardown (0E8B:F826..F990):
  g_PlayerDied := (end ticks = 35); g_MissionOutcome := 1, then the mission-specific tests
  (missions 3, 5, 12, 16 compare slot end positions; 17 uses the loss flag)
  stop speaker; wait for the current sound effect; stop music; free the song buffer;
  close the SFX channel (1E2A:0810); Release(heap mark); restore INT 09h (0E8B:F852)
```

### 4.1 Consequences for the port

- **Draw-before-update.** The frame shown after the flip in tick *t* contains the player at the position computed at the end of tick *t − 1* and the mission ships at their tick-*t* positions. For T1 the simulation order is what matters: projectiles (6) see the player position from the end of the previous tick and the mission ships from the previous tick (they move in step 10); mission logic (8, 9) runs before the ships move (10); the player moves (28) after everything else.
- **Every-other-tick enemy fire.** Step 31 runs only when the draw page is 1 at that point (after the toggle), i.e. on alternate ticks. Fire requests raised on the other ticks are discarded in step 32.
- **Hit flash and comms** are presentation, but the comm viewer (15) blocks the loop while open (a modal screen), so ticks stop.
- **The tick wait sits between HUD drawing and the flip.** A port that runs simulation and rendering separately must still keep steps 1–19 and 22–35 in this order within one simulation step.

## 5. Edge cases and original bugs

| Quirk | Original behaviour | Port policy (proposal) |
|---|---|---|
| Player drawn from last tick's position | One-tick visual lag of the player relative to the world | Preserve in Classic (it is what the player saw) |
| Enemy shots only on alternate ticks | Fire requests on the other ticks are lost | Preserve (affects fire rate) |
| Modal screens inside the tick (comm viewer, Esc menu, F5 screen, pause) | Simulation stops while they are open | Preserve as pause states |
| Flight 21 ends after 25 ticks | Fixed scripted flight | Preserve |

## 6. Test vectors

| # | Setup | Expected | Evidence |
|---|---|---|---|
| 1 | C-0002, flight 1, no input, RAM at frame 3543 (+1 s) and 3613 (+2 s) | `g_MissionTicks` 20 → 39 (19 ticks in 70 frames) | C-0002 `mem\` dumps, DS:1718 |
| 2 | Same | Draw page DS:3A30 = 0 at +1 s and 1 at +2 s (odd number of ticks between the dumps) | C-0002 |

## 7. Presentation and audio hooks

Sound effects are started by steps 5, 6, 31 and the mission and HUD routines through `1E2A:006A(kind, source x, source y, listener x, listener y, sfx setting)` (Pascal declaration order; positions are tile coordinates, the listener is the view origin DS:3A36/3A38; `kind` is the shooter's class code, or 0 with source = FFFFh for the player's own gun). The flight song starts once at setup.

## 8. Open questions

- [ ] Q1 (U-041): internals of `0E8B:0984` (projectile movement, hit tests, kill attribution, player primary fire for classes other than 2 and 5).
- [x] Q2: `2404:1426` holds flights 1–9 and `2404:3521` flights 10–20 ([mission-logic](mission-logic.md), RE-6).
- [ ] Q3 (VE): confirm with breakpoints that steps 18 and 19 run once per tick and never call `Crt.Delay` in normal flight (timing §8 M4).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Loop head and back edge | `0E8B:CC7F`, jump back at `0E8B:F842` | static | CONFIRMED |
| Call order in the tick | `0E8B:CC7F..F842` (call sites listed in §3) | static (instruction order and branch structure) | LIKELY |
| Late-draw switch | `0E8B:D16B..D196`, `0E8B:D629..D633`, `0E8B:E030..E03E` | static | LIKELY |
| Alternate-tick enemy fire | `0E8B:F3xx` test of DS:3A30 | static | LIKELY |
| Setup file list | `0E8B:B880..C27x`, strings `0E8B:B78E..B7F9` | static | CONFIRMED |
| Teardown and outcome flags | `0E8B:F826..F990` | static | LIKELY |
| 19 ticks per 70 frames | C-0002 RAM dumps | capture | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-2 (Archaeologist + Spec author) | Initial: complete per-tick order with addresses |
