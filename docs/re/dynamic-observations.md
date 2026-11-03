# Dynamic observations (reference oracle)

Raw facts measured by running the original game in the reference oracle (DOSBox-X + td2192-oracle instrumentation, ADG §11.1). They are **observations, not specs**: they say what the original does under emulation and cite captures, so that Archaeologists can tie them to code and Spec authors can cite them. Interpretation is kept out, or marked as such.

- **Recorded by:** Verifier VE-1, 2026-11-02 (tasks M0-10, M0-11).
- **Captures:** `P:\msdos\TD2192\re-work\captures\oracle\C-NNNN\` (outside the repo; rows in `re-work\captures\MANIFEST.md`). Each folder holds `frames.tsv`, `int21.tsv`, `pit.tsv`, `io_sites.tsv`, `events.tsv`, `fb\` dumps (+ `png\`), `mem\` dumps where requested, `capture\` recordings, the scenario, `oracle.ini` and the exact command line. Formats: `re-work\dosbox\ORACLE.md` §6.
- **Address notation:** `rel` addresses are image-relative `SSSS:OOOO` (CS − (PSP + 10h) : IP), the ADG §7.8 form, for example `file0001.exe:051A:000D`.

## 1. Rig, build and captures

| | |
|---|---|
| Emulator | DOSBox-X 2026.10.01 (commit `f1c4414`) + td2192-oracle patch 1; Release x64, SDL1, `C_DEBUG`/`C_HEAVY_DEBUG`. Binary SHA-256 `e6fa2aea028b1134374f8199e70a7e8aca740d774416b61c9475fd7cb3858a78` for all six captures; a pinned copy is in `re-work\dosbox\bin\oracle-1\`. |
| Preset | **P1**: `ref-play.conf` + `ref-oracle.conf`: `machine=vgaonly`, `core=normal`, `cputype=386`, `cycles=fixed 20000` (unless the table says otherwise), SB16 220h/IRQ 7/DMA 1/HDMA 5, OPL3 (`nuked`), no joystick, `doublescan=false`, `turbo=true`. |
| Guest clock | Fixed at power-on: 1994-11-01 09:00:00. |
| Game copy | `re-work\dosbox\c\TD`, reset to the pristine install before every run (`td.cfg` restored, `tdstatus.dat` removed). |
| Frame | One emulated video frame (DOSBox-X `VGA_VerticalTimer`); 70.07–70.09 Hz in all modes seen here. Frame 1 = first frame after power-on. "ms" = emulated milliseconds since power-on. |

| ID | Scenario (`re-work\dosbox\scenarios\`) | Cycles | What it covers |
|---|---|---:|---|
| C-0001 | `intro-full.txt` | 20000 | `TD1.EXE` with no input: full intro (`file0000.exe`) to the `file0001.exe` configuration screen. FB every 70 frames + tagged scene shots, WAV + DRO + AVI. |
| C-0002 | `e1-to-m1.txt` | 20000 | Power-on to E1 mission 1 flight and the retry screen: intro skipped, configuration, script choice (1), main menu, mission title, hangar, first dialogue, hint, flight. Tagged FB per screen, **every frame of flight 3473–4172**, RAM (0–9FFFFh) + VRAM at flight +1 s and +2 s, WAV + DRO + AVI. |
| C-0003 | `e1-to-m1.txt` | 20000 | Replica of C-0002 (determinism check). |
| C-0004 | `e1-to-m1-portable.txt` | 3000 | Same inputs, anchored only on game events; speed experiment. |
| C-0005 | `e1-to-m1-portable.txt` | 60000 | Same, speed experiment. |
| C-0006 | `m1-key-probe.txt` | 20000 | Into mission 1, then 13 candidate keys held 20 frames each with FB before / while held / after. |

**Determinism (CONFIRMED):** C-0002 and C-0003 are bit-identical: `frames.tsv`, `int21.tsv`, `pit.tsv`, `events.tsv`, `io_sites.tsv`, all 710 framebuffer dumps, both RAM and both VRAM dumps, and every recording (AVI, WAV, DRO). The intro framebuffers of C-0001 also match earlier exploratory runs frame for frame.

## 2. Timeline from power-on to mission 1 (C-0002, cycles 20000)

| Frame | ms | Event |
|---:|---:|---|
| 0 | 1 | `COMMAND` EXECs `TD1.EXE` (text mode) |
| 8 | 111 | `TD1` EXECs `nuffmem.exe` (exits at once), then `file0000.exe` (intro) |
| 83 | 1176 | Mode 13h |
| 308, 408 | | Scenario Esc presses; the intro ends at frame 426 (text mode at 427) |
| 428–430 | 6105–6139 | `TD1` opens `intro.cmf`, creates `tdstatus.dat` (1 byte, 00h), EXECs `file0001.exe` |
| 430–640 | | `file0001.exe` text-mode configuration screen (reads `td.cfg` = 02 02 02 at frame 494); Down ×3 + Enter |
| 640 | 9123 | `td.cfg` rewritten as **02 02 01**; mode 13h at 641; script-choice screen |
| 780 | 11121 | Key `1`; main menu (music + `td.men`, `tdmen.pal`) from frame 797 |
| 1027 | 14659 | Enter (START NEW GAME preselected) |
| 1062 | 15172 | Mission title card (`td-let.cin`) |
| 1426 | 20370 | Hangar scene (`hangar.cmf`, `hangar.scr`) |
| 2179 | 31112 | First dialogue (`td.pal`, `dialogue.dat`, `newfaces.td` ×2) |
| 2603 | 37174 | Esc ("to the action") |
| 2788 | 39817 | Hint screen (`low.cmf`, `secret.pic`) |
| 3352 | 47875 | Space; mission 1 loads at frames 3402–3437 |
| **3473** | **49608** | **First flight frame**: display height becomes 192 lines |
| ~5500–5771 | | With no flight input the player ship is destroyed; retry screen from 5772 |

Without input, the intro runs to its end at frame 9353 (133.4 s; C-0001), and then `TD1` EXECs `file0001.exe` at frame 9357. In both paths `TD1` writes `tdstatus.dat` = 00h.

## 3. Asset load order, power-on to mission 1 (C-0002)

From `int21.tsv` (`python tools/scripts/oracle-summary.py <run> --loads`). Read pattern: `R768 R200x320` = one 768-byte read then 200 reads of 320 bytes; `seek(m2)` = INT 21h 42h method 2 (from end), and so on. Callers are image-relative.

| # | Frame | Program | Caller | File | Read pattern |
|---:|---:|---|---|---|---|
| 1 | 82 | FILE0000 | 09FD:0983 | `td.cfg` (×2: open/close, then 2 reads) | R2×1 |
| 2 | 82 | FILE0000 | 09FD:0983 | `td.lt2` | R950 |
| 3 | 82 | FILE0000 | 09FD:0983 | `td.shp` | R20480 |
| 4 | 85 | FILE0000 | 09FD:0983 | `intro.map` | R40000 |
| 5 | 86 | FILE0000 | 09FD:0983 | `td.blk` (81,920 bytes) | seek(m1) seek(m2) seek(m0) R232×352 (81,664 bytes; last 256 not read) |
| 6 | 112 | FILE0000 | 09FD:0983 | `td.dun` | R96×32 |
| 7–11 | 112 | FILE0000 | 02FA:3677 | `whoosh.voc`, `pulse.voc`, `ric3.voc`, `scratch.voc`, `home.voc` | each: seek(m2) seek(m0) seek(m1), one read of the whole file |
| 12–14 | 113 | FILE0000 | 02FA:3677 | `intro.cmf`, `intro1.cmf`, `intro2.cmf` | same pattern |
| 15 | 114 | FILE0000 | 09FD:0983 | `intro0.scr` | R768 R200×320 |
| | 1550 / 7593 / 8438 | FILE0000 | 09FD:0983 | `td.pal`, `intro1.scr`, `intro2.scr` (full intro only, C-0001) | |
| 16 | 428 | TD1 | 00CD:34A1 | `intro.cmf` | seek ×3, R9096 |
| 17 | 430 | TD1 | 0717:0983 | `tdstatus.dat` | open fails (err 2), create, W1 (00h) |
| 18 | 439 | FILE0001 | 3A9A:0A14 | `tdstatus.dat` (×3) | open/close; open, seek(m1) seek(m2) seek(m0), R1 (00h); create (truncates to 0 bytes) |
| 19 | 494 | FILE0001 | 3A9A:0A14 | `td.cfg` (×2) | R3×1 |
| 20 | 640 | FILE0001 | 3A9A:0A14 | `td.cfg` (create) | W3×1: 02 02 01 |
| 21 | 640 | FILE0001 | 3A9A:0A14 | `dialogue.dat` | open/close, no read |
| 22 | 640 | FILE0001 | 3A9A:0A14 | `td.lt2`, `td-let.cin` | R950; R2496 |
| 23 | 797 | FILE0001 | 2C37:3658 | `menu.cmf` | seek ×3, whole file |
| 24 | 799 | FILE0001 | 3A9A:0A14 | `td.men` (144,000 bytes) | seek to 42880, R32×320; seek to 16000, R128×320; seek to 80000, R80×100 |
| 25 | 817 | FILE0001 | 3A9A:0A14 | `tdmen.pal`; `td.men` | R768; seek to 16000, R200×320 |
| 26 | 868, 1062 | FILE0001 | 3A9A:0A14 | `td.lt2`; `td-let.cin` | R950; R2496 |
| 27 | 1426 | FILE0001 | 2C37:3658 | `hangar.cmf` | seek ×3, whole file |
| 28 | 1430 | FILE0001 | 3A9A:0A14 | `td.lt2`, `hangar.scr` | R950; R768 R200×320 |
| 29 | 2179 | FILE0001 | 3A9A:0A14 / 0F48 | `td.lt2`, `td.pal`, `dialogue.dat` | R950; R768; R4×128 (file kept open to frame 2788) |
| 30 | 2181, 2183 | FILE0001 | 3A9A:0A14 | `newfaces.td` (435,968 bytes) ×2 | R768 R16000 at offset 0; then seek to 16768, R768 R16000 |
| 31 | 2788 | FILE0001 | 2C37:3658 | `low.cmf` | seek ×3, whole file |
| 32 | 2792 | FILE0001 | 3A9A:0A14 | `td.lt2`, `secret.pic` (11,008 bytes) | R950; **R768 R32×320** |
| 33 | 3402–3404 | FILE0001 | 3A9A:0A14 | `td.pal`, `td.sc3`, `td.lt2`, `td.sc2`, `td.fac`, `td.shp`, `td.exp`, `td.wea`, `td.shr` | R768, R14400, R950, R14400, R3920, R20480, R7168, R3888, R32000 |
| 34 | 3404 | FILE0001 | 3A9A:0A14 | `m1.td` | **R4×20 R2×10000** |
| 35 | 3405 | FILE0001 | 3A9A:0A14 | `td.map`; `td.blk` | R40401; seek ×3, R232×352 |
| 36 | 3429 | FILE0001 | 3A9A:0A14 | `td.win` | R10240×**1** (10,240 one-byte reads) |
| 37 | 3436–3437 | FILE0001 | 2C37:3658 | `missile.voc`, `hornet.voc`, `afterb.voc`, `income.voc`, `boomboom.voc`, `zeus.voc`, `vii.voc`, `vulturei.voc`, `stiletto.voc`, `punyexp.voc`, `td01.cmf` | seek ×3, whole file each |
| 38 | 5774 | FILE0001 | 3A9A:0A14 | `td.lt2`, `td.pal` (retry screen) | R950; R768 |

Two call sites do all file opens of each program: the game's own (FILE0000 `09FD:0983`, FILE0001 `3A9A:0A14`, read-write mode 02h) and the sound library's (FILE0000 `02FA:3677`, FILE0001 `2C37:3658`, read-only mode 00h, always preceded by a size probe with three seeks). File names are passed exactly as listed (sound files partly in upper case, for example `hornet.VOC`, `td01.CMF`).

## 4. PIT programming (C-0001, C-0002)

All writes go to channel 0, mode 2, lo/hi byte access.

| Frame | Program | Site (rel) | Count | Rate | Context |
|---:|---|---|---:|---:|---|
| 18 | FILE0000 | 02FA:4AB9–4AC0 | 2386 | 500.076 Hz | Program start, before any music file is loaded |
| 135 | FILE0000 | same | 12428 | 96.0076 Hz | 22 frames after the intro CMF files are read |
| 426 | FILE0000 | same | 65535 | 18.2068 Hz | Program exit |
| 428 | TD1 | 00CD:48E3–48EA | 2386 | 500.076 Hz | After the intro, `TD1` loads `intro.cmf` |
| 430 | TD1 | same | 65535 | 18.2068 Hz | Before EXEC `file0001.exe` |
| 441 | FILE0001 | 2C37:4A9A–4AA1 | 2386 | 500.076 Hz | Program start |
| 798, 1427, 2789, 3438 | FILE0001 | same | 12428 | 96.0076 Hz | One frame after each CMF load (`menu`, `hangar`, `low`, `td01`) |

With the full intro (C-0001) the same 96.0076 Hz value is written again at frames 1550 and 7439. Count 12428 and the CMF header field at 0x0C (96 in every file, FORMATS §14) agree; see U-007. The DOS clock (INT 21h 2Ch) keeps advancing at normal speed while the PIT runs at 96 Hz (about 1.05 s of DOS time per emulated second over 1-second windows in flight, C-0002).

## 5. Per-screen timing (C-0001, C-0002; cycles 20000)

`python tools/scripts/oracle-summary.py <run> --cadence --range A-B:label`. "Retrace hit" = a frame in which at least one 3DAh read returned bit 3 set. "Gap" = frames between consecutive retrace-hit frames.

| Screen (capture, frames) | IRQ 0 rate | Frames with 3DAh reads / with a hit | Gap mean / mode | 3DAh reads per polling frame | DOS clock calls / frame | 3C9h writes / frame | Full-palette frames (≥768) |
|---|---:|---|---|---:|---:|---:|---:|
| TD1 launcher, text (C-0002, 10–81) | 445 Hz (18.2 Hz to frame 17, then 500 Hz) | 0 / 0 | – | – | 0 | 0 | 0 |
| Intro scene 0 (C-0001, 115–1549) | 101.8 Hz (500 Hz to frame 135, then 96 Hz) | 188 / 187 | 7.61 / 1 | 10,569 | 557.5 | 70.2 | 16 |
| Intro desert flight (C-0001, 1551–7592) | 96.00 Hz | 1936 / 1797 | **3.36 / 4** | 6,197 | 496.3 | 16.5 | 16 |
| Intro scene 1 (C-0001, 7594–8437) | 96.00 Hz | 158 / 158 | 5.24 / 1 | 10,480 | 514.4 | 118.5 | 15 |
| Intro scene 2 (C-0001, 8439–9353) | 95.98 Hz | 160 / 158 | 5.68 / 1 | 10,428 | 521.3 | 109.7 | 16 |
| Configuration screen, text (C-0002, 445–639) | 500.3 Hz | 0 / 0 | – | – | 0 | 0 | 0 |
| Script choice (C-0002, 650–796) | 500.6 Hz | 29 / 29 | 5.21 / 1 | 9,297 | 0 | 151.1 | 3 |
| Main menu (C-0002, 840–1026) | 95.95 Hz | 19 / 19 | 1.78 / 2 | 6,861 | 0 | 59.8 | 0 |
| Mission title (C-0002, 1063–1425) | 95.89 Hz | 34 / 34 | 10.97 / 1 | 9,114 | 554.6 | 70.4 | 3 |
| Hangar scene (C-0002, 1449–2178) | 96.11 Hz | 34 / 34 | 22.09 / 2 | 7,568 | 557.9 | 35.0 | 0 |
| Dialogue (C-0002, 2190–2602) | 96.00 Hz | 257 / 257 | 1.61 / 1 | 11,201 | 0 | 42.3 | 1 |
| Hint screen (C-0002, 2796–3351) | 96.01 Hz | 540 / 539 | 1.03 / 1 | 11,280 | 0 | 22.8 | 0 |
| **Flight** (C-0002, 3473–5771) | 96.01 Hz | 734 / 677 | **3.37 / 4** | 6,039 | 417.5 | 15.2 | 3 |
| Retry screen (C-0002, 5780–6399) | 96.09 Hz | 68 / 68 | 1.22 / 1 | 10,120 | 0 | 21.0 | 0 |

Facts that stand out:

- Static screens (script choice, main menu, dialogue at rest) do no polling at all once drawn; the counts above come from their fade-ins and text output.
- Screens that animate on a timer (intro scenes, mission title, hangar, flight) poll the DOS clock (INT 21h AH=2Ch) between retrace waits, hundreds of times per frame, always from one call site per program: FILE0000 `09F5:003D`, FILE0001 `3A7E:003D`.
- A retrace wait shows as thousands of 3DAh reads with exactly one read returning bit 3 set (it waits for the *start* of vertical retrace).
- DOSBox-X answers AH=2Ch from the BIOS tick count (INT 1Ah), so the clock the game polls advances in 18.2 Hz steps, as on real DOS.

## 6. Flight timing and CPU speed (U-001)

Same inputs, 1,800 flight frames from the first flight frame:

| Capture | Cycles | First flight frame | Frames from `td01.cmf` open to first flight frame | IRQ 0 | Retrace-hit frames | Gap mean / mode | 3DAh reads per polling frame | DOS clock calls / frame |
|---|---:|---:|---:|---:|---:|---|---:|---:|
| C-0004 | 3000 | 4505 | 172 | 96.00 Hz | 389 | **4.61 / 5** | 3,527 | **0.2** |
| C-0002 | 20000 | 3473 | 36 | 96.03 Hz | 527 | **3.41 / 4** | 6,049 | 433.2 |
| C-0005 | 60000 | 3178 | 20 | 96.02 Hz | 521 | **3.46 / 4** | 6,524 | 1,544.8 |

Every frame of the first 10 s of flight was dumped in C-0002 (frames 3473–4172; `oracle-summary.py --fb-cadence flight`):

- The displayed image changed in **178 of 700** frames. Frames between changes: 3 (58×), 4 (95×), 7 (13×), 8 (6×), 1 (5×); mean 3.94.
- 173 of the 178 changes appear in the frame **immediately after** a retrace-hit frame (4 two frames after, 1 in the same frame).
- 202 retrace-hit frames in those 700 frames (20.2 per second).
- A 9-entry palette update (9 × 3C8h, 27 × 3C9h writes) recurs about once per update cycle.

Main sites (`io_sites.tsv`, C-0002): 3DAh reads FILE0001 `051A:000D` (11.3 M reads, frames 2219–5830), `386E:1A8D` (4.3 M, 816–5773), `051A:0085`, `051A:01AF`, `386E:19AC`; `0E8B:0367` only during mission start (3439–3470) and `0E8B:0595` only around the ship's destruction (5722–5755). DAC writes FILE0001 `386E:190A` (3C8h) + `386E:190F/1913/1917` (3C9h); FILE0000 `0957:01C3` + `0957:01C8/01CC/01D0`. FILE0000 3DAh sites: `0000:000D`, `0000:002E`, `0000:00C8`, `0957:0279`, `0957:036B`.

*Reading (not a fact):* at 20000 and 60000 cycles the flight update rate is the same (about 20 retrace-paced updates per second) and the extra CPU time goes into DOS-clock polling, so in that range the flight loop looks paced by the DOS clock plus a retrace wait. At 3000 cycles there is almost no idle polling and updates slow to about 15 per second, so the game is CPU-bound there. The 18.2 Hz clock steps alone would give one update every 3.85 frames, not 3.4; the code has to explain the difference.

## 7. Speed-dependent input handling (C-0002, C-0004, C-0005)

The scenario presses Esc at `file0000.exe` start + 300 and + 400 frames (frames 308 and 408 in every run).

- At 20000 cycles the intro ends at frame 426, after the second Esc.
- At 3000 cycles (C-0004) it ends at frame 406, and at 60000 cycles (C-0005) at frame 403: after the **first** Esc. The second Esc (frame 408) is then still in the keyboard buffer when `file0001.exe` starts, and its configuration screen accepts the configuration without the Down/Enter keys (C-0004 frame 498, C-0005 frame 485; `td.cfg` written 02 02 01 as in C-0002). The scenario's later Down ×3 + Enter land on the script-choice screen and have no visible effect; the run then continues as in C-0002.

So whether the first Esc ends the intro depends on CPU speed, and not monotonically. Scenarios should press one key at a time and anchor the next key on a game event.

## 8. Display geometry in flight (U-025)

- In flight DOSBox-X draws a **320×192** display (draw height 192 from the first flight frame, 3473, until the retry screen, 5772). The refresh rate stays at 70.07 Hz.
- Over frames 3473–4172 (C-0002), rows **0–159** change (the scrolling playfield) and rows **160–191** form the HUD band. Within the band, rows 160–165 and 186–191 never changed and rows 166–185 changed in 8–92 of 699 frame pairs (gauges, message line).

## 9. Files the game writes (U-004, `td.cfg`)

- `tdstatus.dat` is not shipped. `td1.exe` creates it after the intro (open fails with error 2, create, write 1 byte = **00h**, close) immediately before EXEC `file0001.exe`. That held with and without skipping the intro (C-0001, C-0002). `file0001.exe` opens it, takes its size by seeking (1 byte), reads the byte, closes, then **re-creates it empty** (0 bytes) at frame 439.
- `td.cfg` (shipped 02 02 02): `file0000.exe` reads 2 bytes, `file0001.exe` reads 3 bytes, and leaving the configuration screen rewrites it as **02 02 01**. The screen showed the control field as keyboard only while the file held 02 (no joystick is emulated in P1).
- In C-0001 to C-0006 nothing else is written (apart from one write to standard output); no saves or `.stt` files change.
- No EXEC of `td2.exe` or `td3.exe` occurs: `td1.exe` EXECs `nuffmem.exe`, `file0000.exe`, `file0001.exe` only (U-005).

## 10. Reference screenshots

Raw 320×200 (or 320×192 / 720×400) framebuffer dumps, palette + indexed pixels (ORACLE.md §6.6); PNGs next to them in `fb\png\`.

| Screen | Capture | Frame | File |
|---|---|---:|---|
| Intro scene 0 (+1 s, +10 s) | C-0001 | 184, 814 | `fb\f00000184_320x200_intro-planet-1s.vga`, `fb\f00000814_..._intro-planet-10s.vga` |
| Intro desert flight (+2 s, +20 s) | C-0001 | 1690, 2950 | `..._intro-desert-2s.vga`, `..._intro-desert-20s.vga` |
| Intro scenes 1 and 2 | C-0001 | 7733, 8578 | `..._intro-scene1.vga`, `..._intro-scene2.vga` |
| Configuration screen (text mode) | C-0001, C-0002 | 9427, 500 | `..._720x400_config-screen.vga` |
| Script choice | C-0002 | 710 | `fb\f00000710_320x200_script-choice.vga` |
| Main menu | C-0002 | 997 | `fb\f00000997_320x200_main-menu.vga` |
| Mission title | C-0002 | 1162 | `fb\f00001162_320x200_mission-title.vga` |
| Hangar | C-0002 | 1630 | `fb\f00001630_320x200_hangar.vga` |
| First dialogue (3 stages) | C-0002 | 2283, 2433, 2573 | `..._dialogue-1.vga`, `-2`, `-3` |
| Hint screen (text still typing) | C-0002 | 3332 | `fb\f00003332_320x200_hint.vga` |
| First flight frame | C-0002 | 3473 | `fb\f00003473_320x192_flight.vga` (every frame to 4172) |
| Retry screen | C-0002 | 6300 | `fb\f00006300_320x200_retry-screen.vga` |

Audio: C-0001 `capture\td1_000.wav` (whole run, 48 kHz stereo) and `file0000_000.dro` (OPL register stream); C-0002 likewise to the retry screen. Video: ZMBV AVIs in `capture\`, split at video-mode changes.

## 11. Flight key probe (U-024, C-0006)

Thirteen keys (Up, Down, Left, Right, Left Ctrl, Left Alt, Space, Insert, keypad +, Tab, Left Shift, Enter, F6) were each held for 20 frames from frame 4100 onwards, 70 frames apart. A framebuffer was taken the frame before, while held (+19) and 40 frames after release. First look only: **Tab** visibly switches one HUD panel between the before and held frames, and it stays switched. The other keys coincide with enemy fire and explosions in the same frames, so their effects are not yet attributed. The RAM-diff method (ADG §7.9) on a similar probe should settle them.

## 12. Caveats

- Everything here is DOSBox-X's model of a 386 at `cycles=fixed 20000` unless stated. Rates that depend on CPU speed (polling counts, load times, update rate below about 20,000 cycles) are not hardware facts.
- DOSBox-X adds a small simulated overhead to each INT 21h AH=2Ch call. That affects how many polls fit in a frame, not the clock values.
- The intro and menus were not tested with SB Pro or without sound; P1 is SB16. A one-time comparison with `sbtype=sbpro2` is still owed (ADG §11.1).
