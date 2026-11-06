# Hit flash (distance flash)

| | |
|---|---|
| **Spec ID** | `sim/hit-flash` |
| **Revision** | 3 |
| **Status** | Draft |
| **Fidelity tier** | T0 for the DAC values of every tick; T1 for when the flash starts and how it decays (frame parity depends on it). It feeds only the palette: no gameplay state reads `g_HitFlash` |
| **Confidence** | CONFIRMED for the formula, the decay and the palette (static, E1 `0E8B:06C5`, `0E8B:DA6B..DE36`; dynamic: E1 kills in C-0009, C-0023, C-0033 and the E3 end sequence in C-0041, whose code matches E1 instruction for instruction) |
| **Tasks** | M6-07, M7 (feeds IM-7); resolves U-300 |
| **Unknowns** | U-300 (resolved here); U-082 / OB-009 (shared overflow) |

Related: [weapons §4.3](weapons.md) (kill path), [tick-order](tick-order.md) steps 6, 11 and 14, [flow/flight-screens §4.5](../flow/flight-screens.md) (palette formulas per mode), [audio/sfx-policy §4.1](../audio/sfx-policy.md#41-distance) (the same distance formula).

## 1. Summary

When a ship explodes near the player, the low palette entries brighten for a few ticks and fade back. The brightness is the u16 global `g_HitFlash` (DS:12C4). Only two things raise it: a mission ship destroyed by a shot (tick-order step 6), and the player's own death explosion (end-sequence step at end tick 12, step 11). Both call `Sim_DistanceFlash(TileX, TileY)` (`0E8B:06C5`). It computes the distance in whole tiles from the player's tile to the given tile, and raises `g_HitFlash` to `20 − Round(√(dx² + dy²))` if that is larger. Every tick, the palette step (tick-order step 14) first lowers `g_HitFlash` by 2 (by 3 in palette mode 1) and then writes the DAC. The flash adds `g_HitFlash` to red **and green** (not blue) of entries 0..45 except 36 and 38, so it reads as an orange-yellow tint, not pure red. When the value reaches 0, the whole palette is rewritten once.

## 2. Definitions and units

| Term | Original | Type | Unit / range | Notes |
|---|---|---|---|---|
| `g_HitFlash` | DS:12C4 | u16 | 0..20 between ticks (never above 20; 0 = no flash) | E2 DS:11E4, E3 DS:11E4 |
| Player tile | (`g_ViewTileX` + 5, `g_ViewTileY` + 3) | i16 | tiles | `g_ViewTileX` DS:3A36 (6..183), `g_ViewTileY` DS:3A38 (7..190), both i16; the same reference as the player's hit box ([weapons §2](weapons.md)) |
| `TileX`, `TileY` | arguments | u8 | tiles | Pascal order: `TileX` is the **first** parameter (pushed first). The symbol table had `(TileY, TileX)` before this revision |
| `dx`, `dy` | — | 16-bit | tiles | player tile − argument tile |
| `sq` | — | 16-bit, read as i16 | tiles² | `(dx·dx + dy·dy) mod 65536` |
| Candidate `v` | — | i16 | ≤ 20 | `20 − Round(Sqrt(sq))` |
| Palette mode | `g_PaletteMode` DS:1704 | u8 | 0 normal, 1 IR, 2 red | [flight-screens §4.5](../flow/flight-screens.md) |
| White flash | `g_FlashFrames` DS:1828 | u8 | 0..4 | Cancels the hit flash (§4.3) |

## 3. Data

| Constant | Value | Unit | Provenance |
|---|---|---|---|
| Maximum flash | 20 | DAC steps (6-bit) | `0E8B:0731` |
| Player tile offset | +5 X, +3 Y | tiles | `0E8B:06FD`, `0E8B:06D9` |
| "No position" marker | `TileX` = FFh → distance 0 (flash 20) | — | `0E8B:06C9` |
| Decay per tick | 2 (modes 0 and 2), 3 (mode 1) | — | `0E8B:DA82`, `0E8B:DD2F`; `0E8B:DBC8` |
| Reset threshold | value > 30 (u16) after the decay → 0 | — | `0E8B:DA87`, `DBCE`, `DD34` |
| Flashed entries | 0..45 except 36 and 38 | DAC index | `0E8B:DAF8..DB3E` (the list also names 85h, 86h, 90h, C1h, which "≤ 45" already excludes: U-225) |
| End-sequence flash tick | end ticks = 12 (value before the step's increment) | — | `0E8B:5761..5778` |

## 4. Behaviour

### 4.1 `Sim_DistanceFlash(TileX, TileY)` (`0E8B:06C5`)

```text
if TileX = 255:
    d := 0
else:
    dx := (g_ViewTileX + 5) − TileX          16-bit arithmetic
    dy := (g_ViewTileY + 3) − TileY
    sq := (dy·dy + dx·dx) mod 65536          low words of the products; the sum wraps
    sq := sq read as a signed 16-bit value, widened to LongInt
    d  := Round(Sqrt(Real(sq)))              Real48 Sqrt and Round (core/real48 §4.9, §4.6)
    d  := low 16 bits of d, as i16
v := 20 − d
if v > 0 and g_HitFlash < v: g_HitFlash := v
```

- The flash only rises: `g_HitFlash := max(g_HitFlash, v)`. It is never lowered here.
- Several kills in one tick each call it; the largest candidate wins.
- **Integer equivalent.** For 0 ≤ sq ≤ 32767, `Round(Sqrt(sq))` is the k with k² − k + 1 ≤ sq ≤ k² + k (k = 0 for sq = 0). No integer square root lies within 6·10⁻⁴ of a half-integer in this range, far more than the Real48 error, so the port may use either form. Only k ≤ 19 gives a flash:

| sq | 0 | 1–2 | 3–6 | 7–12 | 13–20 | 21–30 | 31–42 | 43–56 | 57–72 | 73–90 | 91–110 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| v | 20 | 19 | 18 | 17 | 16 | 15 | 14 | 13 | 12 | 11 | 10 |

| sq | 111–132 | 133–156 | 157–182 | 183–210 | 211–240 | 241–272 | 273–306 | 307–342 | 343–380 | ≥ 381 |
|---|---|---|---|---|---|---|---|---|---|---|
| v | 9 | 8 | 7 | 6 | 5 | 4 | 3 | 2 | 1 | none |

- **16-bit square (shared with OB-009).** The sum of squares is 16 bits wide. With the view limits (player tile X 11..188, Y 10..193) and ship tiles 0..200, |dx| ≤ 189 and |dy| ≤ 193, so the true dx² + dy² can reach about 73,000:
  - **32,768..65,535** (a true distance of about 181 to 256 tiles): `sq` is negative and `Sqrt` stops the program with RunError 207. This never happens first here. Caller 1 (§4.2) runs after two `Snd_PlayEffect` calls with the same ship tile (hit sound 9, destruction sound 100), which do the same computation (sfx-policy §4.1) and would already have stopped; caller 2 always has sq = 1. So the case belongs to OB-009 / U-082. **Port:** follow the OB-009 fix and treat a negative `sq` as "no flash".
  - **65,536..65,916**: `sq` wraps to 0..380 and a very distant kill flashes as if it were close. Preserve this (T1). It is practically unreachable.
- The `TileX` = FFh branch is a guard: neither caller passes FFh in normal play (live ships stay on the map, and `g_ViewTileX` + 5 ≤ 188).

### 4.2 Callers (all episodes)

| # | Where | Tick-order step | Arguments | When |
|---|---|---|---|---|
| 1 | `0E8B:11E1` in `Sim_ProjectilesAndHits` | 6 | the hit ship's slot tile (`g_SlotTileX[s]` DS:3B2D + s, `g_SlotTileY[s]` DS:3B41 + s) | [weapons §4.3](weapons.md) step 4: a shot (any owner) hits live slot s and leaves its armour < 1. Order: hit sound (9), damage, then destruction sound (100), **then this call**, then armour := 0 and the rest of the kill. Kills by the homing sweep (step 3, kind 6, ±40 px) and kills done by mission scripts (class or armour written directly) do **not** flash |
| 2 | `0E8B:5778` in `Sim_EndSequenceStep` | 11 | (`g_ViewTileX` + 5, `g_ViewTileY` + 2), low bytes | the end-sequence step that starts with `g_EndTicks` = 12, before the player-destroyed sound (100) and the big explosions at the player. dx = 0, dy = 1, so sq = 1 and **v = 19** |

There are no other writers that raise the value. E2 and E3 have the identical routine and the same two call sites: `td2:0E96:06C9`, called from `0E96:11DE` and `0E96:572E`; `td3:0C5D:06C1`, called from `0C5D:11CF` and `0C5D:571A`.

Writers that set `g_HitFlash := 0` (E1):

| Where | When |
|---|---|
| `0E8B:BEFA` | flight setup (with palette mode := 0) |
| `0E8B:0417`, `0E8B:0644` | every level of the flight fade-in and fade-out ([flight-screens §4.5.3](../flow/flight-screens.md)); so the screens that fade end any flash: the computer (Esc) menu and the flight-17 cut-scene. The comm viewer does not fade and keeps the flash (C-0058 tick 318: 10 after the transmission screen, VE-2) |
| `0E8B:B229`, `0E8B:B2BA` | Alt (infra-red toggle) |
| `0E8B:9FA7`, `0E8B:A1EF`, `0E8B:A31F` | mission events of flights 20, 18 and 17 ([mission-logic §5.20, §5.18, §5.17](mission-logic.md)) |
| `0E8B:DE33` | the palette step itself, while a white flash runs (§4.3) |

### 4.3 Decay and palette (tick-order step 14, `0E8B:DA6B..DE36`)

Step 14 runs every tick right after the explosions (step 13), with no condition. The palette writes are those of [flight-screens §4.5.4](../flow/flight-screens.md); the order of the state changes is:

```text
step 14:
  if g_HitFlash > 0 (unsigned) and mode = m:            only the block of the current mode runs
      g_HitFlash := (g_HitFlash − decay(m)) mod 65536    decay = 2 for modes 0 and 2, 3 for mode 1
      if g_HitFlash > 30 (unsigned): g_HitFlash := 0     catches the wrap below zero
      if g_HitFlash = 0: rewrite DAC 0..255 from the mode's "flash ended" formula
      else: for i = 0..45, i ≠ 36, i ≠ 38: DAC i := mode's base colour of P[i] with
            red and green raised by g_HitFlash, each clamped to 63 (blue unchanged; mode 2: red only)
  if g_FlashFrames > 0: g_HitFlash := 0                  after the hit-flash writes of this tick
  white-flash frames 4..1, then g_FlashFrames −= 1        (flight-screens §4.5.4)
```

Consequences:

- Both setters run earlier in the same tick (steps 6 and 11), so the value is lowered once before it is first shown. The first visible strength is v − 2 (v − 3 in mode 1). The DAC is written immediately, so the change appears at the next retrace, not at the page flip (step 22).
- Sequences of `g_HitFlash` after step 14, starting from the tick of the call:
  - mode 0 or 2, v = 20: 18, 16, 14, 12, 10, 8, 6, 4, 2, 0. That is 9 flashed ticks, then one tick that rewrites all 256 entries.
  - v = 19: 17, 15, …, 3, 1, 0.
  - mode 1, v = 20: 17, 14, 11, 8, 5, 2, 0.
  - v = 1 or 2 (mode 0 or 2), or 1..3 (mode 1): 0 at once. Nothing is flashed, but the full rewrite still happens.
- The full rewrite in IR modes uses the truncating formulas, so after **any** kill within about 19.5 tiles the IR palette switches to the "after a hit flash" variant (OB-029, flight-screens §4.5.1).
- A white flash (homing missile kill in step 3 or 4, `g_FlashFrames` := 4) cancels any hit flash: the hit-flash block may still write entries 0..45 in that tick, but the white frame then overwrites all 256 entries, and `g_HitFlash` is 0 from then on.
- The port keeps `g_HitFlash` as u16 with wrapping subtraction, or saturating subtraction followed by the "> 30 → 0" rule; for values ≤ 20 both give the same results.

## 5. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| Negative 16-bit square | RunError 207 in `Sqrt` (reached first in `Snd_PlayEffect`, OB-009) | fix-all with OB-009: no flash |
| Wrapped square ≥ 65,536 | Distant kill flashes | preserve (unreachable in practice) |
| Flash 1 or 2 shows nothing but rewrites the palette | IR palette changes variant (OB-029) | preserve |
| Exclusion list names 85h, 86h, 90h, C1h | No effect (U-225) | follow the code |
| "Red" flash also raises green | Orange-yellow tint | preserve |

## 6. Test vectors

| # | Setup / input | Expected | Evidence |
|---|---|---|---|
| 1 | E3 flight 8, player destroyed by hazard damage; end-sequence step with end ticks 12 at mission tick 274 (mode 0) | `g_HitFlash` after each tick: 274 → 17, 275 → 15, 276 → 13, 277 → 11, 278 → 9, 279 → 7, 280 → 5, 281 → 3, 282 → 1, 283 → 0 | C-0041, DGROUP every frame f2023–2127 (E3 DS:11E4, DS:31DC) |
| 2 | Same capture, frames 2068 (flash 15) and 2072 (flash 13) | DAC i = (min(63, R + f), min(63, G + f), B) of `g_FlightPalette` for i = 0..45 except 36, 38; every other entry = P[i] (256/256 entries match) | C-0041 fb f2068, f2072 vs DGROUP DS:1300 |
| 3 | Same capture, frame 2064 (before) and 2100 (after) | DAC = P (except E3 entry 144, which another E3 effect changes) | C-0041 |
| 4 | E1 flight 1, view (102, 104) (player tile (107, 107)); slot 1 destroyed by a mission-ship shot at (103, 110) in tick 254, mode 0: dx 4, dy −3, sq 25, v 15 | `g_HitFlash` 13 (tick 254), 11, 9, 7, 5, 3, 1, 0 (tick 261) | C-0009 f4396–4421 (DS:12C4) |
| 5 | E1 flight 8, mode 1, same view; slot 15 destroyed at (110, 122): sq 234, v 5; slot 1 at (113, 122): sq 261, v 4 | 2, then 0; 1, then 0 | C-0023 f1747, f1772 (IM-7) |
| 6 | E1 flight 18, mode 0, same view; slot 1 destroyed at (107, 91): sq 256, v 4 | 2, then 0 | C-0033 f1839 (IM-7) |
| 7 | Kill at the player's tile, mode 0 | 18, 16, 14, 12, 10, 8, 6, 4, 2, 0 | static |
| 8 | Kill at dx = 13, dy = 14 (sq 365) | v = 1: 0 in the same tick; full palette rewrite | static |
| 9 | Kill at dx = 14, dy = 14 (sq 392) | no change | static |
| 10 | Mode 1, kill at the player's tile | 17, 14, 11, 8, 5, 2, 0 | static |
| 11 | `g_HitFlash` = 10 after step 14, kill with v = 6 next tick | 10 stays; step 14 → 8 | static |
| 12 | dx = 188, dy = 174 (true sq 65,620) | wrapped sq 84 → v = 11 | static |

## 7. Presentation and audio hooks

No sound of its own; the destruction sound (100) at the ship or at the player comes just before each call (sfx-policy). Palette writes only.

## 8. Open questions

- None for E1. U-082 (whether the negative-square case is reachable) stays open; it is shared with OB-009.

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Formula, argument order, max rule | `file0001:0E8B:06C5..075E`; callers push X first (`0E8B:11D0`, `0E8B:576B`) | static | CONFIRMED |
| Exactly two callers per episode | instruction tables of file0001, td2, td3 (RE-11) | static | CONFIRMED |
| Zeroing writers | `0E8B:0417`, `0644`, `9FA7`, `A1EF`, `A31F`, `B229`, `B2BA`, `BEFA`, `DE33` | static | CONFIRMED |
| Decay, threshold, flashed entries | `0E8B:DA6B..DE36` | static; C-0041 | CONFIRMED |
| End tick 12 → 19 → 17 | `0E8B:5761..5778`; C-0041 | static + dynamic (E3) | CONFIRMED |
| Ship kill → flash, any owner, distance from the player's tile | C-0009, C-0023, C-0033 (four mission-ship kills) | dynamic (E1) | CONFIRMED |
| Integer equivalent of Round(Sqrt) | arithmetic argument, §4.1 | analysis | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-05 | RE-11 (Archaeologist + Spec author) | Initial; resolves U-300 |
| 2 | 2026-11-05 | RE-11 | E1 test vectors 4–6 from C-0009, C-0023, C-0033 (found by IM-7; vector 4 re-checked by RE-11). No behaviour change |
| 3 | 2026-11-06 | main session | §4.2: the comm viewer does not end a flash (it does not fade); only the Esc menu and the flight-17 cut-scene do (VE-2, C-0058; docs/parity/M9-playthrough.md open question 3). td2192 already behaved this way |
