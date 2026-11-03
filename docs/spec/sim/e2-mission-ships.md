# Mission ships in Episode 2: setup, first tick and per-tick deltas

| | |
|---|---|
| **Spec ID** | `sim/e2-mission-ships` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (positions, waypoints, AI state and RNG use per tick), as [mission-ships](mission-ships.md) |
| **Confidence** | CONFIRMED: static reading of `td2:0E96:A76E..BCC8` (`Sim_FlyMission` setup) and `td2:2D26:0011` (`AI_UpdateShips`), and two captures (C-0015 E2 flight 1, C-0016 E2 flight 9: per-frame RAM from the mission-file load through tick 60+) |
| **Tasks** | M10-01 (feeds M10-03 and every E2 mission task) |
| **Unknowns** | U-205 (resolved here), U-201 (resolved here), U-243 |

Read this as a **delta against [mission-ships](mission-ships.md) rev 2** (Episode 1). Everything not listed here is identical in `td2.exe`: the ship record layout, the AI unit (`td2:2F63` = E1 `34E6`, code-identical, [episode-deltas §3.1](../../re/episode-deltas.md)), thrust, turning, drift, drag, carry and edge removal, firing and destruction. Addresses are `td2:<seg>:<off>` (ADG §7.8); DGROUP is `36B6`, and the E2 ship record *s* is at DS:3F31 + 429h × *s* (E1 DS:4432 − D8h for *s* = 1).

## 1. Summary

E2 drops E1's setup loop that calls `AI_ChooseWaypoint` for every slot before the first tick (E1 `0E8B:CC63..CC7D`, mission-ships §4.1 step 6). Nothing replaces it. Because every ship starts with its waypoint on its own tile, the per-tick rule "waypoint reached → `AI_ChooseWaypoint`" (mission-ships §4.2 step (b)) makes that first call in **tick 1** instead. In path mode that call only sets the path cursor to 1 and records the start tile as the previous and return point; the waypoint does not move. So **every path-following ship starts moving one tick later than in E1**, and a ship whose first AI call is an attack step keeps cursor 0 until it is back in path mode. No RNG is consumed by the dropped loop (a path step draws nothing), so the RNG stream at the start of tick 1 is the same as E1's would be; the stream diverges only through the later timing.

The per-tick routine `AI_UpdateShips` (`td2:2D26:0011`) loses all of E1's flight-specific cases and gains a prologue that writes a flag nobody reads.

## 2. Definitions and units

As [mission-ships §2](mission-ships.md). "Tick t" is the pass of the flight loop in which `g_MissionTicks` (E2 DS:163C, u32) becomes t; the last setup state is "tick 0". In a capture, the state *after* tick t is the last frame whose counter is t (a frame can end in the middle of `AI_UpdateShips`).

## 3. Data

No new data. The record fields used below: +000h (`fixedDone` in E1, here the "alive" flag of §4.2), +001h/+002h waypoint, +003h/+004h prev, +00Bh pathCursor (u16), +00Dh.. path steps, +3F5h moveDir (mission-ships §3.1).

## 4. Behaviour

### 4.1 Setup (`Sim_FlyMission`, `td2:0E96:A76E..BCC8`)

Steps 1–5 of mission-ships §4.1 are unchanged:

1. `AI_ResetShipRecords` (`td2:2D26:0000` → AI unit), with its 20 `Random(2)` draws for the hand bits;
2. load `m<n>.td2` (every flight 1..20 has a file);
3. shields, armour, path copy, waypoint := path[1], return := waypoint;
4. tile := path[1], heading 6, target heading 6, offsets (16, 16), speeds 0;
5. `Msn_Setup` ([e2-mission-setup](e2-mission-setup.md)).

**Step 6 is gone.** After the start view, palette and music, the setup ends with the tick-timer sample (`0E96:BCC3`) and falls into the loop head (`0E96:BCC8`). The only call of `AI_ChooseWaypoint` in `td2.exe` is the per-tick one at `td2:2D26:036C`. So after setup, for every slot: pathCursor = 0, prev = (0, 0) (cleared by the reset), moveDir = 0, waypoint = tile = path[1].

### 4.2 Per tick (`AI_UpdateShips(flight)`, `td2:2D26:0011`)

Mission-ships §4.2 applies with these changes:

```text
prologue (new, before pass 1):
    for s = 1..20: record[s].+000h := 1 if class(s) != FFh else 0      (2D26:001F..0053)
playerTile := (low byte(viewX + 5), low byte(viewY + 3))                (unchanged)
pass 1: unchanged
pass 2, for each live slot: steps (a)..(j) of mission-ships 4.2, except:
    (b) waypoint = tile → AI_ChooseWaypoint(s)       (no "flight 20, s ≤ 5" exception)
    (e) thrust and target heading for every ship     (no flight-20 sinking)
    (h) removed                                      (no flight-7 pins)
```

- The flight argument is still pushed by the caller, but nothing reads it.
- **The +000h flag has no reader (U-201 resolved).** In the code-identical AI unit, +000h is touched only by the fixed-point mode, which no E1 or E2 code ever enables (mission-ships §4.3.9). Writing it changes nothing a port can observe; a port may skip it. (Evidence: static. Captures C-0015/C-0016 show the flag at 1 for every live slot from tick 1 and 0 for empty slots.)
- **Removed in `Sim_FlyMission`:** the flight-19 block after `AI_UpdateShips` (shield cap 10, slot-19 copy; mission-ships §4.2.4). Mission-ships §4.2.4 has no E2 rows at all.

### 4.3 What the first AI call does (U-205 resolved)

Apply mission-ships §4.3 to the post-setup state of §4.1:

```text
Tick 1, pass 2 (b), for each live slot s in slot order (waypoint = tile = path[1], cursor 0):
    if the mission logic of tick 1 (steps 8–9, before AI_UpdateShips) gave s an attack order:
        AI_ChooseWaypoint(s) runs an attack step: fire decision (1 Random draw), waypoint moves one tile
        toward the target; pathCursor stays 0
    else (path mode):
        PathStep with c = 0: next step = path[1] = waypoint → dx = dy = 0;
        prev := waypoint; return := waypoint; waypoint unchanged; pathCursor := 1
        (pathCursor := 1 also when path[2].x = 0)
        SetMoveDir: waypoint = prev → moveDir stays 0
    (c)–(g): goal = waypoint = tile → no thrust, no target-heading change
Tick 2, (b): waypoint still = tile → AI_ChooseWaypoint(s) again:
    PathStep with c = 1 → step toward path[2], pathCursor := 2 when reached in one step
```

Consequences an implementation must reproduce:

1. **One-tick delay.** Compared with E1, every ship that starts in path mode reaches each later waypoint exactly one tick later. Do not run E1's setup loop for E2.
2. **Ships spawned later** (class FFh at setup, given a class by mission logic) behave the same way at their first live tick: E1 had already set their cursor to 1 at setup, E2 has not, so their first AI call is again the null step.
3. **Ships ordered to attack before their first path step** keep cursor 0. When they return (return point = path[1], set in setup step 3), the next path step is again the null step at path[1] (cursor 0 → 1), then they head for path[2]. In E1 they would head for path[2] directly.
4. **RNG.** The dropped loop drew nothing, so `RandSeed` at the top of tick 1 equals the value after setup (C-0015, C-0016). Every draw in tick 1 comes from attack steps (one fire-decision draw each), in slot order inside pass 2, plus whatever else the E1 tick order draws ([tick-order](tick-order.md)).
5. **Empty slots retyped by `Msn_Setup`** (OB-035, E2 flight 2 slot 8: class 3 at tile (0, 0)) take part in tick 1 like any live ship: the prologue flags them, and their first AI call is the null path step before the edge rule removes them (U-243).

## 5. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| Missing setup loop (U-205) | One-tick delay of every path start; null first path step | preserve (it is the E2 behaviour, not a bug) |
| +000h written every tick, never read | §4.2 | may be omitted (no observable effect) |
| OB-035 | E2 flight 2 slot 8 is a class-3 ship at (0, 0) for one tick | preserve ([e2-mission-setup §5](e2-mission-setup.md)) |
| OB-013..OB-017 | AI-unit quirks of E1 | carry over unchanged (identical code) |

## 6. Test vectors (captures C-0015 and C-0016, no player input)

| # | Capture | Check | Result |
|---|---|---|---|
| 1 | C-0015, E2 flight 1, tick 0 (last setup frame 664) | live slots 1, 3, 4, 6..13, 15, 16: pathCursor 0, prev (0, 0), moveDir 0, waypoint = tile = path[1]; +000h = 0 | as stated (e.g. slot 1: tile (20, 89), path[2] (19, 89)) |
| 2 | C-0015, tick 1 (frame 666) | path-mode slots 1, 4, 6..12, 15, 16: cursor 1, prev = waypoint = tile, moveDir 0; +000h = 1 | as stated |
| 3 | C-0015, tick 1 | slots 3 and 13 are given attack orders by flight 1's engagement rules ([e2-mission-logic §5.1](e2-mission-logic.md): 3 → 7 within 20 tiles (|dx| 19, |dy| 7), 13 → 1 within 15 (10, 9); no other pair is in range): slot 3 waypoint (21, 159) → (22, 158), slot 13 (30, 98) → (29, 97), both moveDir 8, cursor 0 | as stated; `RandSeed` advances by exactly 2 LCG steps in tick 1 (05322D58h → 24BCB19Eh), one per attack step |
| 4 | C-0015, tick 2 (frame 670) | path-mode slots step toward path[2]: slot 1 waypoint (19, 89), prev (20, 89), cursor 2, moveDir 4, target heading 4; slot 7 (40, 153), moveDir 2 | as stated; no RNG draw in tick 2 |
| 5 | C-0016, E2 flight 9 (map set 2), ticks 0–2 | same pattern for all 11 live slots (2, 3, 5, 8..15): tick 1 cursor 1 with the waypoint unchanged, tick 2 first step toward path[2] (e.g. slot 2 (103, 37) → (103, 38), slot 5 (1, 30) → (2, 30)); no engagement, so no RNG draw in ticks 1 and 2 (`RandSeed` stays 05322D58h) | as stated |

`RandSeed` after setup is 05322D58h in both captures: the program is started directly with the oracle's fixed clock, and the unit-initialisation and setup draws are the same. It is a fact of these captures, not a constant to hard-code.

## 7. Open questions

- [ ] U-243: E2 flight 2 slot 8 (OB-035) in tick 1: not yet captured.
- [ ] Long-run E2 ship behaviour has not been compared with a model beyond tick 2 (the E1 model of mission-ships §6 should apply unchanged, with §4 above).

## 8. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| No setup `AI_ChooseWaypoint` loop | `td2:0E96:BCC3..BCC8`; sole caller `td2:2D26:036C` | static + C-0015/C-0016 | CONFIRMED |
| Prologue +000h flag | `td2:2D26:001F..0053` | static + captures | CONFIRMED |
| +000h has no live reader | AI unit identical to E1; mission-ships §4.3.9 | static | LIKELY |
| Per-flight cases removed | episode-deltas §5.2 (`td2:2D26:0011` vs E1 `3292:0011`) | static | CONFIRMED |
| First-tick behaviour | §4.3 derived from mission-ships §4.3.5 | C-0015 (flight 1), C-0016 (flight 9) | CONFIRMED |

## 9. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-9 (Archaeologist + Spec author + Verifier) | Initial: E2 setup without the waypoint loop, first-tick behaviour (U-205), prologue flag (U-201), removed per-flight cases; vectors from C-0015 and C-0016 |
