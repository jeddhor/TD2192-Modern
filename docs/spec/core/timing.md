# Timing model

| | |
|---|---|
| **Spec ID** | `core/timing` |
| **Revision** | 3 (draft, static analysis only) |
| **Status** | Draft |
| **Fidelity tier** | T1 for simulation steps per game tick (one fixed step per tick); T2 for the wall-clock tick rate and frame pacing |
| **Confidence** | LIKELY (static). Needs the dynamic measurements in §8 before it can be CONFIRMED |
| **Tasks** | M1-06 (static part done; dynamic part for VE), feeds M4-02, M6-01 |
| **Unknowns** | U-001, U-007, U-032, U-033 |

## 1. Summary

In flight, Episode 1 runs one simulation-and-render iteration per **game tick** and paces ticks by **busy-waiting on the DOS clock**. At the end of each iteration, `Sim_FlyMission` (`file0001:0E8B:B880`) calls DOS Get Time (INT 21h AH=2Ch) repeatedly until the time in hundredths has moved more than 4 past the previous tick's timestamp. DOS time only advances when the BIOS timer tick fires, so in practice **one game tick = one BIOS timer tick**. The BIOS tick is nominally 18.2065 Hz, but the WORX sound library reprograms the PIT and chains the BIOS handler at a slightly different average rate (≈ 18.52 Hz when WORX is idle, ≈ 19.20 Hz while a 96 Hz CMF song plays). Nothing compensates for a slow CPU: if an iteration takes longer than a tick, the game simply runs slower. Rendering happens once per iteration and is shown by a page flip that waits for vertical retrace (70.086 Hz), so the display updates at the tick rate, not at 70 Hz.

## 2. Definitions and units

| Term / quantity | Type in original | Unit | Range | Notes |
|---|---|---|---|---|
| Game tick | — | — | — | One pass of the in-flight loop in `Sim_FlyMission` |
| BIOS tick | — | — | — | One increment of the BIOS tick count (0040:006C) by the INT 08h chain |
| DOS time sample | 4 × Word (h, m, s, 1/100) | — | — | `g_TickPrevTime` DS:1708..170E, `g_TickNowTime` DS:1710..1716 |
| Timestamp `T` | Word, wraps | 1/100 s | 0..65535 | `((h×60 + m)×60 + s)×100 + hs`, every product truncated to 16 bits (it wraps every 655.36 s) |
| PIT input clock | — | Hz | 1,193,182 | Standard |
| Fast-tick divisor `d` | Word | PIT counts | | PIT channel 0, mode 2 (control word 34h) |
| Chain interval `N` | Word | fast ticks | | `N = 65535 div d`; WORX calls the old INT 08h every N fast ticks |

## 3. Data

| Constant | Value | Unit | Provenance |
|---|---|---|---|
| Tick limiter threshold | `T_now > T_prev + 4` (unsigned 16-bit) | 1/100 s | `file0001:0E8B:E099` (`add ax,4`) / `0E8B:E0C2` |
| WORX install divisor | 0952h = 2386 → 500.08 Hz; N = 27 → BIOS ≈ 18.52 Hz | | `file0001:2C37:330B` |
| WORX song divisor | 1234DCh div *clock*, *clock* = CMF header word 0x0C (ticks per second; 96 in every shipped song) → d = 12428, 96.01 Hz; N = 5 → BIOS ≈ 19.20 Hz | | `file0001:2C37:47EF..47FA` ([cmf §4.1](../audio/cmf.md#41-clock)) |
| WORX shutdown divisor | FFFFh (≈ 18.21 Hz, N = 1) | | `file0001:2C37:4270` |
| VGA mode 13h refresh | 70.086 Hz | Hz | VGA timing (not from the binary) |
| DOS time step per BIOS tick | 5 or 6 hundredths (5.49 on average) | 1/100 s | DOS converts the tick count to time |

| Variable | Address | Type | Meaning (confidence) |
|---|---|---|---|
| `g_TickPrevTime` | DS:1708 | 4 × u16 | Timestamp of the previous tick (LIKELY) |
| `g_TickNowTime` | DS:1710 | 4 × u16 | Latest sample in the wait loop (LIKELY) |
| `g_MissionTicks` | DS:1718 | u32 | Ticks since the flight started, +1 at the top of every tick (CONFIRMED: C-0002 RAM 20 → 39 between frames 3543 and 3613). 25 ends flight 21; 200 limits the IR notice; 15000 is used by `0E8B:8E87`/`9860` |
| WORX fast-tick counter | CS:31BC in `2C37` | u16 | Counts up to `N`, then chains the BIOS handler (CONFIRMED) |
| WORX chain interval `N` | CS:31BE in `2C37` | u16 | `65535 div d` (CONFIRMED) |
| WORX re-entry guard | CS:42DC in `2C37` | i16 | Starts at −1 (HYPOTHESIS); the ISR's second-level work runs only when not re-entered |
| WORX missed-tick counter | CS:44ED in `2C37` | u16 | Incremented when the second-level work was busy (LIKELY) |

## 4. Behaviour

### 4.1 In-flight loop pacing (`Sim_FlyMission`)

Our reading of the tail of the per-tick loop (`file0001:0E8B:E041..E12C`):

```text
each game tick, after the simulation and drawing of this tick:
  if joystick is available and the controller setting is "joystick": read joystick   (1EBF)
  call 0E8B:9860                         # status panel, messages, ship switching (sim/tick-order.md)
  call 0E8B:8E87                         # instruments: gauges, radar, guidance line
  repeat                                 # Sim_TickLimiter, 0E8B:E05C
    now  := DOS time as 16-bit hundredths (wrapping arithmetic)
    prev := g_TickPrevTime as 16-bit hundredths
    if now > prev + 4 (unsigned, 16-bit): break
    if now < prev (unsigned):             break   # clock wrapped
  g_TickPrevTime := g_TickNowTime        # the new tick starts now
  ... (loop continues with the next tick)
```

- The simulation is not given the elapsed time. Each iteration advances the game by one fixed step (LIKELY: no use of the timestamps other than the wait was found).
- **Fast machine:** the wait ends at the first BIOS tick after the previous one, so there is exactly one iteration per BIOS tick.
- **Slow machine:** when an iteration takes longer than a tick, the wait exits at once, so the iteration rate (and game speed) drops. There is no catch-up.
- **Quirks:** the 16-bit timestamp wraps every 655.36 s, and `prev + 4` can overflow when `prev ≥ 65532`. Either way the wait exits immediately, giving one short tick about every 11 minutes. Midnight rollover does the same.

### 4.2 Where the BIOS tick comes from

WORX (`file0001:2C37`) installs an INT 08h handler (`WORX_TimerIsr`, `2C37:4558`) when it initialises (`2C37:3285`):

```text
on each PIT channel-0 interrupt (rate 1,193,182 / d Hz):
  if music or sound flags (CS:31C4 & 16h) are set: run the WORX music/sound tick (2C37:487E)
  fast_count += 1
  if fast_count >= N:
      fast_count := 0
      call the saved BIOS INT 08h handler     # BIOS tick count +1, EOI, INT 1Ch
  else:
      EOI; run second-level work (2C37:44EF) unless re-entered
```

So the effective BIOS tick (and with it DOS time and the game tick) runs at `1,193,182 / (d × N)` Hz with `N = 65535 div d`:

| WORX state (static reading) | d | Fast rate | N | BIOS / game tick rate | Period |
|---|---:|---:|---:|---:|---:|
| WORX not installed (if that happens with sound off: §8) | 65536 | 18.21 Hz | 1 | 18.2065 Hz | 54.93 ms |
| Installed, no song | 2386 | 500.08 Hz | 27 | 18.521 Hz | 53.99 ms |
| Song with clock = 96 ticks/s | 12428 | 96.01 Hz | 5 | 19.202 Hz | 52.08 ms |
| Song tempo changes (`2C37:48BB`) | varies | | `65535 div d` | varies | |

The game's tick rate therefore depends on the **sound state**: the Classic port must choose one canonical rate (see §5). The clock is CMF header word 0x0C (U-007, resolved): every shipped song gives the 19.20 Hz row.

Which row applies in flight (static, `file0001:0000:4585..47FF`, `0E8B:B898..C92B`; U-032):

| Configuration | WORX timer installed? | Music in flight | Game tick |
|---|---|---|---|
| `/nosound` or `/pc` as the first parameter | **no** (a flag at DS:13DC skips all WORX set-up) | none | 18.2065 Hz |
| FM music enabled (`/sb`, `/a`, or `td.cfg`/`/config` with music device 2) | yes | always: `td01`/`td03`/`td02` for mission number mod 3 = 1/2/0 ([music-cues](../audio/music-cues.md)) | ≈ 19.20 Hz |
| WORX installed, music device ≠ 2 (from `td.cfg`) | yes | none | ≈ 18.52 Hz |

The in-flight song is started after the mission set-up and runs for the whole mission, so the 96 Hz divisor holds throughout the flight loop. A song is never stopped during flight.

### 4.3 Frame presentation

Drawing goes to a hidden page of the unchained VGA memory. `Gfx_FlipPage` (`386E:1A6C`) then waits for display-enable to go low, sets the CRTC start address, and waits for the start of vertical retrace. `Sim_FlyMission` calls it 3 times and `Gfx_SetSplitLine` (`386E:19A0`, which also waits for a full retrace) once, so a tick costs at least one retrace wait (≤ 14.3 ms). That fits easily within a 52–55 ms tick, so on a fast machine the retrace wait does not reduce the tick rate. It only phase-aligns presentation. **Display rate = tick rate ≈ 18–19 Hz.**

### 4.4 Outside flight

- Screens, menus and story sequences (`051A`, `0000`) pace themselves with Crt `Delay` (calibrated busy loop, `3A1C:02A8`, 30 call sites) and retrace-synchronised fade loops (one palette step per retrace: `0000:0000`/`0124`, `051A:0016`/`013F`, `0E8B:01F6`/`0426`).
- `051A:1AA3` calls DOS Get Time 3 times (purpose unknown, U-033).
- The sound-effect unit (`1E2A`) hooks INT 1Ch, so its handler runs at the BIOS tick rate above, not at a fixed 18.2 Hz.

### 4.5 Per tick vs per frame

There is one frame per tick: draw, flip and simulate all happen once per loop iteration. The order of the work inside the tick is in [sim/tick-order.md](../sim/tick-order.md) (U-033): the world is drawn into the back page first, then the HUD, then the tick wait and the flip, and **player input and player physics run after the flip**, at the end of the tick. `g_MissionTicks` (DS:1718) is incremented once at the top of every tick (CONFIRMED: 19 per 70 frames in C-0002).

## 5. Edge cases and original bugs

| Quirk | Original behaviour | Port policy (proposal; MDG §6.3) |
|---|---|---|
| Speed depends on CPU when the CPU is too slow | Game slows down, no catch-up | Classic: fixed tick, never slows (fix-all); slowdown is not a "feel" players rely on |
| Tick rate depends on WORX state (≈ 18.2 / 18.5 / 19.2 Hz) | Slightly different game speed with music, without music, and without sound hardware | Classic: one canonical rate. Proposal: the rate measured in the most common configuration (SB + music), expected ≈ 19.20 Hz. Decide after §8 measurements (ADR) |
| 16-bit timestamp wrap every 655.36 s | One short tick about every 11 minutes | Do not reproduce (no state effect beyond one early tick) |
| Display updates at tick rate (~19 Hz) | Visible stepping on scrolling | Classic: present at the tick rate. Enhanced: optional interpolation, presentation only |

## 6. Test vectors

| # | Setup / input | Expected output | Evidence |
|---|---|---|---|
| 1 | Static: WORX idle (d = 2386) | BIOS and game tick ≈ 18.52 Hz | static arithmetic |
| 2 | Static: song at 96 ticks/s | BIOS and game tick ≈ 19.20 Hz | static arithmetic |
| 3 | Dynamic: DOSBox, E1M1 in flight, cycles 8,000 / 20,000 / 60,000, SB + music | `g_MissionTicks` per second: 19.21 / 19.21 / 19.23 (frame-quantised, ±0.02); 19.208 over 378.5 s at 20,000 | C-0074, C-0010 (VE-3) |
| 4 | Dynamic: same with `/nosound` | 18.21 / 18.22 per second at 20,000 / 60,000: WORX does not install its timer (IRQ 0 stays at 18.21 Hz) | C-0074 (VE-3) |
| 5 | Dynamic: SB, music device ≠ 2 (`td.cfg` 01 02 02) | 18.52 / 18.53 per second at 20,000 / 60,000 (IRQ 0 500.28 Hz) | C-0072, C-0074 (VE-3) |
| 6 | Dynamic: cycles 3,000 | 13.35 (music), 13.39 (no music), 13.90 (`/nosound`) per second: CPU-bound, no catch-up | C-0074 (VE-3) |

## 7. Presentation and audio hooks

- Music ticks come from the WORX INT 08h handler at the CMF clock rate, independent of the game tick.
- The sound-effect unit's INT 1Ch handler runs at the BIOS tick rate (§4.2).

## 8. Open questions and measurements for the Verifier

Runtime address = (PSP + 10h + seg):off, where PSP is the program's PSP segment (ADG §7.8).

Results (VE-3, 2026-11-06; method: `g_MissionTicks` and the BIOS data area dumped every frame instead of breakpoints, so the timing is undisturbed; C-0072, C-0074, C-0075, C-0009, C-0010; details in [M12-timing](../../parity/M12-timing.md) §2):

- [x] M1 (U-001): §6 vectors 3–6. 19.21 / 18.52 / 18.21 Hz at 8,000–60,000 cycles; 13.3–13.9 Hz at 3,000 (CPU-bound). `cycles=max` is not deterministic in the oracle and was replaced by 60,000.
- [x] M2 (U-032): PIT writes in `pit.tsv`: d = 2386 at WORX install, 12428 at every song start (also at loop restarts), none under `/nosound`. IRQ 0: 500.28 / 96.01 / 18.21 Hz. The WORX words were not read.
- [x] M3: BIOS tick count `0040:006C`: 19.20–19.22 Hz with music, 18.51–18.54 without, 18.21 with `/nosound`, equal to the M1 rates at ≥ 8,000 cycles. The game tick starts in the frame of the BIOS tick (74–85 %) or the next one (after the retrace wait).
- [~] M4 (U-033): not broken on. Indirect: frames per tick are 2–5 (3.65 mean) with no stalls in 617 ticks of flight at each fast setting, so `0E8B:9860` does not wait in normal flight.
- [~] M5: +1 per tick in every frame-by-frame run (617 ticks × 9 runs, 1,131 frames of C-0009). Tick 200: see flight-screens (IR notice). Tick 15,000 not reached: flight 1 parked with difficulty 0 ended at tick 14,766 (C-0075, U-403).
- [x] M6: one retrace-hit frame per tick in 93 % of ticks (a single retrace wait, so one flip); the others have a second frame with 20–60 retrace hits, the end-of-flight and destruction sequences.
- [x] M7 (U-024): no chaining for ordinary keys. In flight `q`, `z`, `x` and keypad 8 (C-0074, `/nosound` 20,000) leave the BIOS keyboard buffer head/tail (`0040:001A/1C`) and shift flags unchanged; before the flight the Esc on the mission card moved them.
- [x] Q (U-007): resolved. The word read at `2C37:47EF` is CMF header offset 0x0C (ticks per second).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Tick wait on DOS time, threshold > 4 hundredths, wrap exit | `file0001:0E8B:E05C..E12C` | static | LIKELY (code read; that it runs once per tick is inferred from its position) |
| `Sim_FlyMission` is the in-flight routine | `file0001:0E8B:B880`, 21 calls from `Main_CampaignLoop` `0000:0F06`; installs INT 09h | static | LIKELY |
| WORX INT 08h handler chains every N = 65535 div d fast ticks | `file0001:2C37:4558..45BA`, `2C37:4A72..4AA6` | static | CONFIRMED |
| WORX divisors 0952h / 1234DCh div clock / FFFFh | `2C37:330B`, `2C37:47F2`, `2C37:4270` | static | CONFIRMED |
| Page flip waits for retrace | `file0001:386E:1A6C..1A93` | static | CONFIRMED |
| One frame per tick, no elapsed-time scaling | `0E8B:B880` structure | static | HYPOTHESIS (loop body not fully mapped) |
| Rates in §4.2 | arithmetic from the divisors | derived | LIKELY |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-1 (Archaeologist + Spec author) | Initial static draft |
| 2 | 2026-11-02 | RE-2 (Archaeologist + Spec author) | Names for `0E8B:9860`/`8E87`; per-tick order moved to `sim/tick-order.md` (U-033); `g_MissionTicks` confirmed per tick |
| 3 | 2026-11-02 | RE-4 (Archaeologist) | U-007 resolved (header 0x0C); which tick rate applies in which sound configuration (U-032, static) |
| 3 + evidence | 2026-11-06 | VE-3 (Verifier) | §6 vectors 3–6 and §8 M1–M7 measured (no rule changed). The canonical Classic rate is a proposal in parity/M12-timing.md §3, for an SP revision and ADR |
