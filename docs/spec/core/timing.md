# Timing model

| | |
|---|---|
| **Spec ID** | `core/timing` |
| **Revision** | 4 |
| **Status** | Draft |
| **Fidelity tier** | T1 for simulation steps per game tick (one fixed step per tick); T2 for the wall-clock tick rate and frame pacing |
| **Confidence** | CONFIRMED: the tick mechanism and the three rates of §4.2 by static reading and measurement (§6 vectors 3–6, C-0074, C-0010); the clocks outside flight (§4.4) by static reading and C-0002, C-0067, C-0170. The canonical Classic rate is a decision, proposed in [ADR-0007](../../adr/0007-classic-tick-rate.md) (19.2015 Hz) and pending the user's acceptance |
| **Tasks** | M1-06, M12-02; feeds M4-02, M6-01 |
| **Unknowns** | U-001, U-007, U-032 (resolved), U-033 |

## 1. Summary

In flight, Episode 1 runs one simulation-and-render iteration per **game tick** and paces ticks by **busy-waiting on the DOS clock**. At the end of each iteration, `Sim_FlyMission` (`file0001:0E8B:B880`) calls DOS Get Time (INT 21h AH=2Ch) repeatedly until the time in hundredths has moved more than 4 past the previous tick's timestamp. DOS time only advances when the BIOS timer tick fires, so in practice **one game tick = one BIOS timer tick**. The BIOS tick is nominally 18.2065 Hz, but the WORX sound library reprograms the PIT and chains the BIOS handler at a slightly different average rate: measured 19.21 Hz while a 96 Hz CMF song plays (the shipped configuration), 18.52 Hz when WORX is idle (music off) and 18.21 Hz under `/nosound` (§4.2, §6). Nothing compensates for a slow CPU: if an iteration takes longer than a tick, the game simply runs slower. Rendering happens once per iteration and is shown by a page flip that waits for vertical retrace (70.086 Hz), so the display updates at the tick rate, not at 70 Hz.

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

The game's tick rate therefore depends on the **sound state**: the Classic port must choose one canonical rate (see §5). Measured at preset P1 from 8,000 to 60,000 cycles (C-0074, C-0010): 19.21 Hz (19.208 over 378.5 s), 18.52 Hz and 18.21 Hz for the three rows, equal to the divisor products within the oracle's clock error. CONFIRMED. The clock is CMF header word 0x0C (U-007, resolved): every shipped song gives the 19.20 Hz row.

Which row applies in flight (static, `file0001:0000:4585..47FF`, `0E8B:B898..C92B`; U-032):

| Configuration | WORX timer installed? | Music in flight | Game tick |
|---|---|---|---|
| `/nosound` or `/pc` as the first parameter | **no** (a flag at DS:13DC skips all WORX set-up) | none | 18.2065 Hz |
| FM music enabled (`/sb`, `/a`, or `td.cfg`/`/config` with music device 2) | yes | always: `td01`/`td03`/`td02` for mission number mod 3 = 1/2/0 ([music-cues](../audio/music-cues.md)) | ≈ 19.20 Hz |
| WORX installed, music device ≠ 2 (from `td.cfg`) | yes | none | ≈ 18.52 Hz |

The in-flight song is started after the mission set-up and runs for the whole mission, so the 96 Hz divisor holds throughout the flight loop. A song is never stopped during flight.

### 4.3 Frame presentation

Drawing goes to a hidden page of the unchained VGA memory. `Gfx_FlipPage` (`386E:1A6C`) then waits for display-enable to go low, sets the CRTC start address, and waits for the start of vertical retrace. `Sim_FlyMission` calls it 3 times and `Gfx_SetSplitLine` (`386E:19A0`, which also waits for a full retrace) once, so a tick costs at least one retrace wait (≤ 14.3 ms). That fits easily within a 52–55 ms tick, so on a fast machine the retrace wait does not reduce the tick rate. It only phase-aligns presentation. **Display rate = tick rate ≈ 18–19 Hz.**

### 4.4 Clocks outside flight (BUG-7)

Every timed wait outside the flight loop uses one of three clocks. Which one matters, because they scale differently: DOS time follows the BIOS tick of §4.2, `Crt.Delay` is CPU time calibrated in real milliseconds, and retrace waits follow the 70.086 Hz display.

**1. DOS time.** `Flow_WaitSecondsOrKey(n)` (`file0001:051A:1AA3`; the E2 and E3 copies, E3 at `04A0:0269`) polls DOS Get Time until the centisecond clock has advanced by 100·n or a key or button arrives ([screens §2.2](../flow/screens.md#22-waits-u-123)). DOS time is derived from the BIOS tick count: in the oracle, centiseconds of the day = floor(ticks × 6,553,600 / 1,193,182) (5.4925 per tick, in steps of 5 or 6), and it moves only when a BIOS tick arrives. So:

```text
a wait of n DOS seconds started during BIOS tick T0 ends at the first BIOS tick T with
    cs(T) − cs(T0) ≥ 100·n,      cs(T) = floor(T × 6,553,600 / 1,193,182)
=> T − T0 = ceil(n × 18.2065) or one less: 91 or 92 ticks for 5 s, 182 or 183 for 10 s
=> wall time ≈ n × 18.2065 / f_BIOS seconds; with the Classic clock f_BIOS = 19.2015 Hz (ADR-0007):
   n × 0.94818 s   (5 s ≈ 4.74 s ≈ 332 frames; 10 s ≈ 9.48 s ≈ 664 frames)
```

A port either keeps a free-running BIOS tick counter at f_BIOS from program start and evaluates `cs` from it (exact, including the 91/92 variation; the intro already models this clock for its flyover, [intro](../flow/intro.md)), or waits round(n × 18.2065) ticks of 1/f_BIOS (within one tick). Measured: 5 s holds of 328 frames between the fades (C-0002 f1081..1408 and C-0170 f1036..1364, `menu.cmf` playing) and a 10 s hold of 664 frames (C-0002 f1483..2146, `hangar.cmf`), against 4.69..4.74 s and 9.43..9.48 s predicted.

The flight loop's tick limiter (§4.1) reads the same clock. In the original, with music off these waits run at 18.52 Hz (n × 0.983 s) and under `/nosound` at 18.21 Hz (n s); Classic uses 19.2015 Hz for all (ADR-0007; a T2 deviation, §5). The 16-bit wrap quirk of the wait (OB-034) is unchanged.

**2. `Crt.Delay(ms)`** (Borland Crt, `3A1C:02A8`): a busy loop calibrated once, at Crt's unit initialisation, against one BIOS tick. That runs before WORX installs its timer (and `td1` puts the PIT back to 65535 before it starts a child: C-0002 `pit.tsv`, frame 430), so the calibration sees the standard 18.2 Hz tick and `Crt.Delay(ms)` lasts ms real milliseconds (T2). It does **not** scale with the BIOS tick. C-0067: the flight-17 cut-scene (7,300 ms of delays, four 34-frame fades, 28,800 one-byte file reads) takes 673 frames (9.60 s), 7.66 s besides the fades: real milliseconds plus the reads.

**3. Vertical retrace** (70.086 Hz). Palette fades wait for the retrace bit once per level, and how many frames a level takes depends on the work per level ([screens §2.1](../flow/screens.md#21-fades): 2 frames for a full palette, 1 for a nearly black one, at P1). Typed text waits once per character (one character per frame, C-0002). The menu highlight ramps wait once per step but the wait returns at once while the bit is still set, so they make 11 steps per frame (C-0002, C-0120, C-0125). The flight menu's item fade behaves the same way: 32 steps in 3 frames (C-0012 f3628..3630, one retrace-bit hit per step).

Every timed wait in Episode 1, by clock (E2 and E3 use the same routines: [e2-program-flow](../flow/e2-program-flow.md), [e3-program-flow](../flow/e3-program-flow.md)):

| Wait | Clock | Length | Site |
|---|---|---|---|
| Mission title card | DOS time | 5 s | `051A:5940` |
| Scene cards (13 backgrounds) | DOS time | 10 s | `051A:2029` … `5EC2` ([text §4.5.1](../render/text.md#451-episode-1-scene-cards)) |
| `eggbase` scene card | DOS time | 15 s | `051A:40BD` |
| Story-still sets (`td1-13`, `td1-14`, `td1-16`, `td1-20`) | DOS time | 25 s | `051A:46ED` |
| `td1-18` still | DOS time | 15 s | `051A:4952` |
| `td1-15` still | DOS time | 10 s | `051A:4A49` |
| Assignment orders | DOS time | 5 s | `051A:6516` |
| Enemy-ace dossier | DOS time | 10 s | `051A:67FA` |
| Debrief, after its line delays | DOS time | 10 s | `051A:73FF` |
| Pilot dossier (end of game), after its line delays | DOS time | 25 s | `051A:7686` |
| Story so far, pages 1 and 2 | DOS time | 30 s, 45 s | `051A:903F`, `9223` |
| Flight tick | DOS time | one BIOS tick | `0E8B:E05C` (§4.1) |
| Debrief lines | `Crt.Delay` | 4 × 500 ms | `051A:715C`, `71E8`, `723D`, `73E8` |
| Pilot dossier lines | `Crt.Delay` | 5 × 500 ms and 1 × 2000 ms | `051A:75C5` … `766F` |
| Save and restore screens | `Crt.Delay` | 200 ms (two sites each) | `051A:7BD6`, `7CB1`, `83D6`, `84B1` |
| Main menu, joystick up or down | `Crt.Delay` | 200 ms | `0000:34C5`, `352B` |
| Configuration screen (text mode, not ported) | `Crt.Delay` | 100 ms, 5000 ms | `0000:4D13`, `4D1F` |
| Joystick calibration | `Crt.Delay` | 10 ms | `0E8B:B167` |
| Flight Esc menu, key poll | `Crt.Delay` | 100 ms per poll | `0E8B:7B4C` |
| Flight 19 V.C.I. screen | `Crt.Delay` | 2000 ms | `0E8B:A167` |
| Flight 17 cut-scene | `Crt.Delay` | 5 × 1000, 3 × 100 and 1 × 2000 ms | `0E8B:A529` … `A6B1` |
| Scene, menu and flight fades | retrace | 17 levels, 1 or 2 frames per level | screens §2.1, [flight-screens §4.5.3](../flow/flight-screens.md#453-flight-fades) |
| Typed text, dialogue lines, comm screen | retrace | one character per frame | [text §4.3](../render/text.md#43-typed-text-txt_typestring-051a1269-flight-0e8b4c9f), flight-screens §4.2 |
| Menu highlight ramps (E1, E2, E3) | retrace | 11 steps per frame | screens §3.1 |
| Flight menu item fade | retrace | 11 steps per frame, 32 steps | flight-screens §4.1 |

All other screens and keys wait without a timeout. `051A:1AA3` is the only routine outside flight that reads DOS time (U-033's three Get Time calls are its own).

The sound-effect unit (`1E2A`) hooks INT 1Ch, so its PC-speaker handler runs at the BIOS tick rate above, not at a fixed 18.2 Hz.

### 4.5 Per tick vs per frame

There is one frame per tick: draw, flip and simulate all happen once per loop iteration. The order of the work inside the tick is in [sim/tick-order.md](../sim/tick-order.md) (U-033): the world is drawn into the back page first, then the HUD, then the tick wait and the flip, and **player input and player physics run after the flip**, at the end of the tick. `g_MissionTicks` (DS:1718) is incremented once at the top of every tick (CONFIRMED: 19 per 70 frames in C-0002).

## 5. Edge cases and original bugs

| Quirk | Original behaviour | Port policy (proposal; MDG §6.3) |
|---|---|---|
| Speed depends on CPU when the CPU is too slow | Game slows down, no catch-up | Classic: fixed tick, never slows (fix-all); slowdown is not a "feel" players rely on |
| Tick rate depends on WORX state (measured 19.21 / 18.52 / 18.21 Hz) | Slightly different game speed with music, without music, and without sound hardware; DOS-time waits scale the same way | Classic: one canonical rate for flight ticks and DOS-time waits, 1,193,182 / 62,140 = 19.2015 Hz (SB + music), **proposed in ADR-0007, pending the user**. The music-off and `/nosound` rates are not reproduced (T2 deviation). `Crt.Delay` stays in real milliseconds and retrace waits at 70.086 Hz |
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
| 7 | `Flow_WaitSecondsOrKey(5)`, music playing (§4.4) | 91 or 92 BIOS ticks: 4.69..4.79 s at 19.2015 Hz; measured hold 328 frames between the fades | C-0002, C-0170 |
| 8 | `Flow_WaitSecondsOrKey(10)`, music playing | 182 or 183 BIOS ticks: 9.43..9.53 s; measured 664 frames | C-0002 |
| 9 | `Crt.Delay`: flight-17 cut-scene (7,300 ms of delays) | 673 frames including four 34-frame fades | C-0067 |

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
- [~] M5: +1 per tick in every frame-by-frame run (617 ticks × 9 runs, 1,131 frames of C-0009). Tick 200: see flight-screens (IR notice). Tick 15,000 not reached: flight 1 parked with difficulty 0 ended at tick 14,766 (C-0075) because it was won (all ships left by their exits while the view stayed at the start; U-403 resolved by RE-16). The 15,000-tick rule is flight 13's only ([mission-logic](../sim/mission-logic.md) §5.13).
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
| Rates in §4.2 | arithmetic from the divisors; C-0074, C-0010 | derived + measured | CONFIRMED |
| DOS-time waits: one routine, centiseconds of DOS time | `051A:1AA3` and its timed callers (§4.4 table); C-0002, C-0170 | static + capture | CONFIRMED |
| DOS clock from the BIOS tick count | the oracle's INT 21h AH=2Ch (DOSBox-X, read as a tool); [dynamic-observations](../../re/dynamic-observations.md) | static (oracle) | CONFIRMED for the oracle |
| `Crt.Delay` calibrated before WORX starts; 31 call sites | Crt init in the unit chain before `0000:47C2`; C-0002 `pit.tsv`; call sites from RE-7's listings | static + capture | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-1 (Archaeologist + Spec author) | Initial static draft |
| 2 | 2026-11-02 | RE-2 (Archaeologist + Spec author) | Names for `0E8B:9860`/`8E87`; per-tick order moved to `sim/tick-order.md` (U-033); `g_MissionTicks` confirmed per tick |
| 3 | 2026-11-02 | RE-4 (Archaeologist) | U-007 resolved (header 0x0C); which tick rate applies in which sound configuration (U-032, static) |
| 3 + evidence | 2026-11-06 | VE-3 (Verifier) | §6 vectors 3–6 and §8 M1–M7 measured (no rule changed). The canonical Classic rate is a proposal in parity/M12-timing.md §3, for an SP revision and ADR |
| 4 | 2026-11-06 | RE-16 (Archaeologist + Spec author) | Measured rates CONFIRMED (§1, §4.2); §4.4 rewritten: the three clocks outside flight, the exact DOS-time rule with the ADR-0007 clock (BUG-7), `Crt.Delay` in real ms, retrace-paced work, and every timed wait of E1 by clock; §5 policy points to ADR-0007 (proposed, pending); vectors 7–9 |
