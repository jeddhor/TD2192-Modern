# Timing model

| | |
|---|---|
| **Spec ID** | `core/timing` |
| **Revision** | 1 (draft, static analysis only) |
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
| WORX song divisor | 1234DCh div *clock* (*clock* = a CMF header field, ticks per second) → for 96: d = 12428, 96.01 Hz; N = 5 → BIOS ≈ 19.20 Hz | | `file0001:2C37:47F2..47FA` |
| WORX shutdown divisor | FFFFh (≈ 18.21 Hz, N = 1) | | `file0001:2C37:4270` |
| VGA mode 13h refresh | 70.086 Hz | Hz | VGA timing (not from the binary) |
| DOS time step per BIOS tick | 5 or 6 hundredths (5.49 on average) | 1/100 s | DOS converts the tick count to time |

| Variable | Address | Type | Meaning (confidence) |
|---|---|---|---|
| `g_TickPrevTime` | DS:1708 | 4 × u16 | Timestamp of the previous tick (LIKELY) |
| `g_TickNowTime` | DS:1710 | 4 × u16 | Latest sample in the wait loop (LIKELY) |
| `g_MissionTicks` | DS:1718 | u32 | Counter compared with 25, 200 and 15000 (HYPOTHESIS: mission ticks elapsed; 15000 ticks ≈ 13.5 min) |
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
  call 0E8B:9860                         # per-tick routine, purpose TBD (messages/HUD?)
  call 0E8B:8E87                         # per-tick routine, purpose TBD
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

The game's tick rate therefore depends on the **sound state**: the Classic port must choose one canonical rate (see §5). Which CMF header field supplies the clock is U-007. The FORMATS survey found that offset 0Ch is 96 in every shipped CMF; if WORX reads that field, every song gives the 19.20 Hz row.

### 4.3 Frame presentation

Drawing goes to a hidden page of the unchained VGA memory. `Gfx_FlipPage` (`386E:1A6C`) then waits for display-enable to go low, sets the CRTC start address, and waits for the start of vertical retrace. `Sim_FlyMission` calls it 3 times and `Gfx_SetSplitLine` (`386E:19A0`, which also waits for a full retrace) once, so a tick costs at least one retrace wait (≤ 14.3 ms). That fits easily within a 52–55 ms tick, so on a fast machine the retrace wait does not reduce the tick rate. It only phase-aligns presentation. **Display rate = tick rate ≈ 18–19 Hz.**

### 4.4 Outside flight

- Screens, menus and story sequences (`051A`, `0000`) pace themselves with Crt `Delay` (calibrated busy loop, `3A1C:02A8`, 30 call sites) and retrace-synchronised fade loops (one palette step per retrace: `0000:0000`/`0124`, `051A:0016`/`013F`, `0E8B:01F6`/`0426`).
- `051A:1AA3` calls DOS Get Time 3 times (purpose unknown, U-033).
- The sound-effect unit (`1E2A`) hooks INT 1Ch, so its handler runs at the BIOS tick rate above, not at a fixed 18.2 Hz.

### 4.5 Per tick vs per frame

There is one frame per tick: draw, flip and simulate all happen once per loop iteration. Which subsystems run in which order inside the loop (input, AI, physics, collisions, missions, HUD) is not mapped yet (U-033, M1-04/M6-01).

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
| 3 | Dynamic (pending): DOSBox, E1M1 in flight, fast cycles, SB + music | Hits on `0E8B:E117` per second ≈ 19.2 | VE capture (to do) |
| 4 | Dynamic (pending): same with `/nosound` | ≈ 18.2 or 18.5 per second (tells whether WORX installs its timer without sound) | VE capture (to do) |

## 7. Presentation and audio hooks

- Music ticks come from the WORX INT 08h handler at the CMF clock rate, independent of the game tick.
- The sound-effect unit's INT 1Ch handler runs at the BIOS tick rate (§4.2).

## 8. Open questions and measurements for the Verifier

Runtime address = (PSP + 10h + seg):off, where PSP is the program's PSP segment (ADG §7.8).

- [ ] M1 (U-001): breakpoint on `file0001:0E8B:E117` (first instruction after the tick wait). Count hits per wall-clock second in flight, E1M1, at `cycles=max` and at fixed cycles (3000, 8000, 20000), with SB+music, SB without music (if selectable) and `/nosound`. Expected 19.2 / 18.5 / 18.2 Hz on a fast setting, and lower at low cycles.
- [ ] M2 (U-032): at those breakpoints read the WORX words `2C37:31BE` (N) and `2C37:31BC`, and log every `OUT 40h` value (the PIT divisor) during flight. Confirm d = 2386 at install and d = 1234DCh div *clock* during a song.
- [ ] M3: rate of the BIOS tick count `0040:006C` during flight (should equal the M1 rate).
- [ ] M4 (U-033): break on `0E8B:9860` and `0E8B:8E87`: are they called once per tick? Does `0E8B:9860` call `Delay` (`3A1C:02A8`) during normal flight, which would stall ticks?
- [ ] M5: watch `DS:1718` (`g_MissionTicks`): does it increase by 1 per tick? What happens at 200 and 15000?
- [ ] M6: count `Gfx_FlipPage` (`386E:1A6C`) calls per tick (expect 1 in steady flight).
- [ ] M7 (U-024): does `Inp_KeyboardIsr` (`0E8B:0653`) ever chain to the BIOS handler (DS:18C2)?
- [ ] Q (U-007): which CMF header field WORX turns into the divisor at `2C37:47EF` (the word at ES:DI after `add di,2`).

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
