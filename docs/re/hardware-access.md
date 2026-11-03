# Hardware access catalogue (`file0001.exe`)

Every `IN`/`OUT`/`INS`/`OUTS`/`INT` site in Episode 1, grouped by device (M1-03). Addresses are image-relative (`SSSS:OOOO`, ADG §7.8). Unit roles are in [bp7-runtime-semantics §3.1](../spec/core/bp7-runtime-semantics.md#31-unit--code-segment-map-file0001exe), and names in [symbols/file0001.csv](symbols/file0001.csv).

**Method:** `tools/ghidra/FindPortIO.java` on the canonical project after `SetupBp7Program` (2026-11-02), plus reading the code at the key sites. The port is taken from the immediate operand, or traced through `mov/inc/dec/add/sub dx`. `INT` sites carry the `AH` value when it is loaded nearby. **466 sites** in total. The appendix table lists them all; the sections below explain the ones that matter for the port.

**Confidence:** the port or interrupt number of each site is CONFIRMED (read from the instruction or a short trace). Purposes are LIKELY unless marked otherwise. A site in code that Ghidra has not reached (about 6% of code-segment bytes, mostly WORX) is not listed; see [EXECUTABLES.md §1.1](EXECUTABLES.md#11-ghidra-image-base-segments-and-dgroup-m0-08-2026-11-02).

## Summary by device

| Device | Sites | Who | What it tells the port |
|---|---:|---|---|
| VGA registers (SEQ 3C4/5h, GC 3CEh, CRTC 3D4/5h) | 344 | Graphics unit `386E` (most), `0E8B`, `051A`, `0000` | The game runs VGA mode 13h **unchained** (planar, mode-X style) with page flipping, hardware scroll offset and a split-screen line compare (below) |
| VGA retrace (3DAh) | 11 | `386E`, `0E8B`, `051A`, `0000` | Page flips and fades wait for vertical retrace. Frame pacing is **not** retrace-locked (see [timing](../spec/core/timing.md)) |
| VGA DAC (3C8h/3C9h) | 4 | `386E:1901` | Palette upload routine (one index write, then R, G, B in a loop) |
| BIOS video INT 10h | 2 | `386E:19EF` (AX=0013h), Crt (AH=02h) | Mode 13h set before unchaining |
| PIT channel 0 (40h/43h) | 3 | WORX only | Timer rate for music; see [timing](../spec/core/timing.md) |
| PIC EOI (20h) | 2 | Keyboard ISR `0E8B:065B`, WORX timer ISR | — |
| Keyboard (60h, INT 16h) | 5 | Game ISR (`IN 60h`); Crt (INT 16h) | Flight input comes from the game's own INT 09h handler. Menus use BIOS `ReadKey` |
| Joystick (201h) | 8 | `1EBF` | Analog timing loop (`Inp_JoystickRead`) |
| PC speaker (61h, 42h, 43h=B6h) | 16 | Crt `Sound`/`NoSound`, WORX | PC-speaker effects |
| DMA (02h/03h/0Ah/0Bh/0Ch/83h) | 14 | WORX | 8-bit DMA channel 1 (page register 83h) for Sound Blaster playback |
| SB DSP (base + n, base in a variable) | 6 | WORX | DSP reset/read/write |
| OPL2 (388h) | 2 | WORX | AdLib detect / register writes |
| MPU-401 (330h) | 1 | WORX | MIDI output (presumably for a General MIDI option) |
| INT 63h | 1 | WORX `2C37:0000` | WORX's own driver API: WORX hooks INT 63h at start-up |
| DOS INT 21h | 47 | System, Dos, Crt, WORX | File I/O, vectors, time, exit, exec |

## Interrupt vectors hooked

| Vector | Installed by | Handler | Old vector saved at | Restored by | Notes |
|---|---|---|---|---|---|
| 08h (timer) | `WORX_InstallInterrupts` `2C37:3285` (INT 21h AH=25h at `2C37:3304`) | `WORX_TimerIsr` `2C37:4558` | CS:0412 (WORX segment) | `WORX_RemoveInterrupts` `2C37:336E` | Chains to the old handler once every N fast ticks |
| 63h (WORX API) | `2C37:3285` (`2C37:32BD`) | `2C37:34EE` | CS:3219 | `2C37:336E` | Pascal side calls it from `2C37:0000` |
| 09h (keyboard) | `Sim_FlyMission` `0E8B:B880` (`Dos_SetIntVec` at `0E8B:C498`, saved by `Dos_GetIntVec` at `0E8B:C489`) | `0E8B:0653` (body from `0E8B:065B`) | DS:18C2 | `0E8B:A714` (`0E8B:A72B`), `0E8B:F852` | Installed per mission, so it is active only in flight |
| 1Ch (BIOS user tick) | `Snd_InstallTick` `1E2A:06C6` (`1E2A:0716`) | `1E2A:0000` | DS:1E34 | `1E2A:0810` (`1E2A:0845`) | Runs at the BIOS tick rate; the sound-effect unit's tick (HYPOTHESIS: PC-speaker effect sequencer) |
| 00h, 1Bh, others | System / Crt start-up (`3A9A:0000`, `3A1C:003B`) | RTL | RTL save area | `BP_Halt` | Divide error → RunError 200; Ctrl-Break flag |

## VGA programming

- **Mode set** (`Gfx_InitUnchained`, `386E:19EF`): INT 10h AX=0013h, then sequencer 04h (chain-4 off), CRTC 14h (dword mode off) and CRTC 17h (byte mode), and all four planes cleared. Result: 320×200, 256 colours, four planes, so several 16,000-byte pages fit in the 64 KB window. (CONFIRMED from the register indices; the exact values written are a rendering-spec detail for M6-07.)
- **Page flip** (`Gfx_FlipPage`, `386E:1A6C`, 47 call sites): wait until display-enable is low (3DAh bit 0), `CLI`, write the CRTC start address (0Ch high, 0Dh low) from the argument, `STI`, then wait until vertical retrace starts (3DAh bit 3). The argument is a byte offset into video memory: for example `5AC0h` at `0E8B:C746`, and a computed value at `0E8B:E1CF`, which suggests hardware scrolling in flight.
- **Split screen** (`Gfx_SetSplitLine`, `386E:19A0`, 40 call sites): wait for the end and then the start of a vertical retrace, then read-modify-write CRTC 18h (line compare), 07h (overflow bit 4) and 09h (bit 6) with the argument (for example `13Fh` at `0E8B:C871`). The VGA then restarts scan-out at address 0 below that line: a fixed status panel under a scrolling playfield (HYPOTHESIS, M6-07).
- **Logical width** (`386E:1991`): CRTC 13h (offset), for a virtual screen wider than 320 pixels (HYPOTHESIS: scrolling map buffer).
- **CRTC 12h** (vertical display end) is written by `0E8B:A714` and `Sim_FlyMission` (`0E8B:B880`).
- **Plane writes:** most `386E` routines set sequencer 02h (map mask) and GC 05h/01h/08h (write mode, enable set/reset, bit mask) around blits. That is the classic latched four-plane copy. `0000`, `051A`, `0E8B` contain small "all planes, full bit mask" set-ups before solid fills.
- **Palette:** a single DAC writer (`386E:1901`, 3C8h then 3×3C9h per entry). No DAC reads (3C7h) were found, so fades must work from a palette copy in memory (M5-02).
- **Retrace waits without a flip:** `Gfx_WaitRetraceStart` (`0E8B:0052`, and an identical near copy at `051A:0000`) waits only for bit 3 to become set, so it returns at once if called during a retrace. Fade loops (`0000:0000`/`0124`, `051A:0016`/`013F`, `0E8B:01F6`/`0426`) step a counter (for example DS:0AE0 0→10h or 10h→0) and wait for retrace once per step.

## Keyboard

`Inp_KeyboardIsr` (`0E8B:0653`): it sets DS to DGROUP, reads port 60h, treats E0h as a prefix (adds 100h to the next code via DS:1CB6), updates the key-state table at DS:1A5C (make → 1, break → 0, LIKELY), sends EOI, and then sets some flags (for example DS:1E30 when DS:1AA1 = 1). It **does not chain** to the BIOS handler while installed (no far call to the saved vector was seen; to confirm). This is the source for U-024 (key bindings): read which `g_KeyDown` indices the flight code tests (M6-01).

## Sound hardware (WORX)

The full API is M1-02's job. Hardware facts found here:

- PIT channel 0 is reprogrammed with control word 34h (mode 2, lo/hi) by `WORX_SetTimerRate` (`2C37:4A72`), which is called with divisor 0952h at install (≈ 500.1 Hz), with 1234DCh ÷ (CMF clock rate) when a song starts (`2C37:4791`), from a tempo routine (`2C37:48BB`), and with FFFFh at shutdown (`2C37:426F`; that call also locks further changes).
- The PC speaker is driven both through Crt `Sound` (`3A1C:02D4`) and through WORX's own channel-2 code (`2C37:3D07..3F5C`, control word B6h, divisor from a table at DS:281B indexed by a CS variable).
- DMA: mask/clear/mode/address/count/page writes for channel 1 (page register 83h) at `2C37:4CF9..4D46`, plus status polling of port 02h (`2C37:4DC4`).
- SB DSP ports are reached through a base-address variable (`2C37:3CAF`, `5D00`, `5E47`): `DX(?)` in the appendix.
- OPL detection or writes at 388h (`2C37:3CB6`, `3CBA`) and an MPU-401 write at 330h (`2C37:5D18`).

## DOS services

System and Dos wrappers account for most `INT 21h` sites (`AH` = 3Dh/3Fh/40h/42h/3Eh file I/O, 25h/35h vectors, 2Ch time, 30h version, 44h IOCTL, 4Bh exec, 4Ch exit). WORX itself reads files (3Fh/42h/3Eh at `2C37:357E`, `35C1`, `3771`) for VOC/CMF loading. The game calls DOS `GetTime` 5 times: 3 in `051A:1AA3` and 2 in `Sim_FlyMission` (the tick limiter, [timing](../spec/core/timing.md)). `Exec` is only reached from `0E8B` (the `label.exe` path, never ported).

## Notes and anomalies

- `INT 63h` is not a hardware or BIOS interrupt: WORX installs its own handler there at start-up and calls it from Pascal (`2C37:0029`).
- Before the 2026-11-02 Setup fixes, Ghidra also reported dozens of `INSB`/`OUTSB`/`IMUL` "sites": that was lowercase text decoded as code, and it is gone now. If such mnemonics reappear in a `FindPortIO` export, re-run `SetupBp7Program`.
- E2/E3 have not been catalogued yet (M1-09). Their WORX, graphics and Crt units are byte-for-byte the same size as E1's, so the same picture is expected.

## Appendix: every site

Routine = containing function (our name where one exists). Sites = offsets within the segment shown. `DX(?)` = port held in a variable or set outside the traced window.

### VGA DAC (palette) (4 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `386E` | `386E:1901` | `OUT 3C8h` | OUT DX,AL | 190A |
| `386E` | `386E:1901` | `OUT 3C9h` | OUT DX,AL | 190F, 1913, 1917 |

### VGA retrace / status (11 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `0000` | `0000:0000` | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 006A |
| `0000` | `0000:0124` | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 018F |
| `051A` | `051A:0000` | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 000D |
| `051A` | `051A:0016` | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 0085 |
| `051A` | `051A:013F` | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 01AF |
| `0E8B` | `Gfx_WaitRetraceStart` (0E8B:0052) | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 0058 |
| `0E8B` | `0E8B:01F6` | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 0367 |
| `0E8B` | `0E8B:0426` | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 0595 |
| `386E` | `Gfx_SetSplitLine` (386E:19A0) | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 19A7 |
| `386E` | `Gfx_FlipPage` (386E:1A6C) | `IN 3DAh` | status read (retrace bit 3 / display-enable bit 0) | 1A76, 1A8D |

### VGA registers (sequencer, graphics controller, CRTC) (344 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `0000` | `0000:330E` | `OUT 3C4h` | SEQ 02h map mask | 3332 |
| `0000` | `0000:330E` | `OUT 3C5h` | map mask data 0Fh (all planes) | 334A |
| `0000` | `0000:330E` | `OUT 3CEh` | GC 08h bit mask | 332C, 335E |
| `051A` | `051A:0269` | `OUT 3C4h` | SEQ 02h map mask | 028D |
| `051A` | `051A:0269` | `OUT 3C5h` | map mask data 0Fh (all planes) | 02AE |
| `051A` | `051A:0269` | `OUT 3CEh` | GC 08h bit mask | 0287, 02CB |
| `051A` | `051A:1618` | `OUT 3C4h` | SEQ 02h map mask | 18B6, 18F0, 191E, 1949 |
| `051A` | `051A:1618` | `OUT 3CEh` | GC 01h enable set/reset | 18C1 |
| `051A` | `051A:1618` | `OUT 3CEh` | GC 05h mode | 18BD |
| `051A` | `051A:1618` | `OUT 3CEh` | GC 08h bit mask | 18C5 |
| `0E8B` | `0E8B:0000` | `OUT 3C4h` | SEQ 02h map mask | 001D |
| `0E8B` | `0E8B:0000` | `OUT 3C5h` | map mask data 0Fh (all planes) | 002C |
| `0E8B` | `0E8B:0000` | `OUT 3CEh` | GC 08h bit mask | 0017, 0046 |
| `0E8B` | `0E8B:0061` | `OUT 3C4h` | SEQ 02h map mask | 00E6, 0117 |
| `0E8B` | `0E8B:0061` | `OUT 3C5h` | map mask data 0Fh (all planes) | 00F6 |
| `0E8B` | `0E8B:0061` | `OUT 3CEh` | GC 08h bit mask | 00E0, 0110 |
| `0E8B` | `0E8B:012A` | `OUT 3C4h` | SEQ 02h map mask | 01B2, 01E3 |
| `0E8B` | `0E8B:012A` | `OUT 3C5h` | map mask data 0Fh (all planes) | 01C2 |
| `0E8B` | `0E8B:012A` | `OUT 3CEh` | GC 08h bit mask | 01AC, 01DC |
| `0E8B` | `Inp_RestoreKeyboard` (0E8B:A714) | `OUT 3D4h` | CRTC 12h vertical display end | A771, AEC5 |
| `0E8B` | `Sim_FlyMission` (0E8B:B880) | `OUT 3D4h` | CRTC 12h vertical display end | CC46 |
| `386E` | `386E:0000` | `OUT 3C4h` | SEQ 02h map mask | 0026, 006C, 00B2, 00F8 |
| `386E` | `386E:0000` | `OUT 3CEh` | GC 01h enable set/reset | 0031, 0077, 00BD, 0103 |
| `386E` | `386E:0000` | `OUT 3CEh` | GC 05h mode | 002D, 0073, 00B9, 00FF |
| `386E` | `386E:0000` | `OUT 3CEh` | GC 08h bit mask | 0035, 007B, 00C1, 0107 |
| `386E` | `386E:012B` | `OUT 3C4h` | SEQ 02h map mask | 0157, 0198, 01D9, 021A |
| `386E` | `386E:012B` | `OUT 3CEh` | GC 01h enable set/reset | 0162, 01A3, 01E4, 0225 |
| `386E` | `386E:012B` | `OUT 3CEh` | GC 05h mode | 015E, 019F, 01E0, 0221 |
| `386E` | `386E:012B` | `OUT 3CEh` | GC 08h bit mask | 0166, 01A7, 01E8, 0229 |
| `386E` | `386E:024A` | `OUT 3C4h` | SEQ 02h map mask | 0297, 02DC, 0312, 0357 |
| `386E` | `386E:024A` | `OUT 3CEh` | GC 01h enable set/reset | 02A2, 031D, 0362 |
| `386E` | `386E:024A` | `OUT 3CEh` | GC 05h mode | 029E, 0319, 035E |
| `386E` | `386E:024A` | `OUT 3CEh` | GC 08h bit mask | 02A6, 0321, 0366 |
| `386E` | `386E:0391` | `OUT 3C4h` | SEQ 02h map mask | 03E0, 0425, 046A, 04AF |
| `386E` | `386E:0391` | `OUT 3CEh` | GC 01h enable set/reset | 03EB, 0430, 0475, 04BA |
| `386E` | `386E:0391` | `OUT 3CEh` | GC 05h mode | 03E7, 042C, 0471, 04B6 |
| `386E` | `386E:0391` | `OUT 3CEh` | GC 08h bit mask | 03EF, 0434, 0479, 04BE |
| `386E` | `386E:04E9` | `OUT 3C4h` | SEQ 02h map mask | 0538, 0583, 05CB, 0613 |
| `386E` | `386E:04E9` | `OUT 3CEh` | GC 01h enable set/reset | 0543, 058E, 05D6, 061E |
| `386E` | `386E:04E9` | `OUT 3CEh` | GC 05h mode | 053F, 058A, 05D2, 061A |
| `386E` | `386E:04E9` | `OUT 3CEh` | GC 08h bit mask | 0547, 0592, 05DA, 0622 |
| `386E` | `386E:0650` | `OUT 3C4h` | SEQ 02h map mask | 06A3, 06EC, 0735, 077E |
| `386E` | `386E:0650` | `OUT 3CEh` | GC 01h enable set/reset | 06AE, 06F7, 0740, 0789 |
| `386E` | `386E:0650` | `OUT 3CEh` | GC 05h mode | 06AA, 06F3, 073C, 0785 |
| `386E` | `386E:0650` | `OUT 3CEh` | GC 08h bit mask | 06B2, 06FB, 0744, 078D |
| `386E` | `386E:07BC` | `OUT 3C4h` | SEQ 02h map mask | 080D, 0856, 089F, 08E8 |
| `386E` | `386E:07BC` | `OUT 3CEh` | GC 01h enable set/reset | 0818, 0861, 08AA, 08F3 |
| `386E` | `386E:07BC` | `OUT 3CEh` | GC 05h mode | 0814, 085D, 08A6, 08EF |
| `386E` | `386E:07BC` | `OUT 3CEh` | GC 08h bit mask | 081C, 0865, 08AE, 08F7 |
| `386E` | `386E:0926` | `OUT 3C4h` | SEQ 02h map mask | 0976, 09C0, 0A0A, 0A54 |
| `386E` | `386E:0926` | `OUT 3CEh` | GC 01h enable set/reset | 0981, 09CB, 0A15, 0A5F |
| `386E` | `386E:0926` | `OUT 3CEh` | GC 05h mode | 097D, 09C7, 0A11, 0A5B |
| `386E` | `386E:0926` | `OUT 3CEh` | GC 08h bit mask | 0985, 09CF, 0A19, 0A63 |
| `386E` | `386E:0A93` | `OUT 3C4h` | SEQ 02h map mask | 0AE5, 0B2F, 0B79, 0BC3 |
| `386E` | `386E:0A93` | `OUT 3CEh` | GC 01h enable set/reset | 0AF0, 0B3A, 0B84, 0BCE |
| `386E` | `386E:0A93` | `OUT 3CEh` | GC 05h mode | 0AEC, 0B36, 0B80, 0BCA |
| `386E` | `386E:0A93` | `OUT 3CEh` | GC 08h bit mask | 0AF4, 0B3E, 0B88, 0BD2 |
| `386E` | `386E:0C02` | `OUT 3C4h` | SEQ 02h map mask | 0C53, 0C9B, 0CE3, 0D2B |
| `386E` | `386E:0C02` | `OUT 3CEh` | GC 01h enable set/reset | 0C5E, 0CA6, 0CEE, 0D36 |
| `386E` | `386E:0C02` | `OUT 3CEh` | GC 05h mode | 0C5A, 0CA2, 0CEA, 0D32 |
| `386E` | `386E:0C02` | `OUT 3CEh` | GC 08h bit mask | 0C62, 0CAA, 0CF2, 0D3A |
| `386E` | `386E:0D68` | `OUT 3C4h` | SEQ 02h map mask | 0DB7, 0DFC, 0E41, 0E86 |
| `386E` | `386E:0D68` | `OUT 3CEh` | GC 01h enable set/reset | 0DC2, 0E07, 0E4C, 0E91 |
| `386E` | `386E:0D68` | `OUT 3CEh` | GC 05h mode | 0DBE, 0E03, 0E48, 0E8D |
| `386E` | `386E:0D68` | `OUT 3CEh` | GC 08h bit mask | 0DC6, 0E0B, 0E50, 0E95 |
| `386E` | `386E:0EC0` | `OUT 3C4h` | SEQ 02h map mask | 0F0F, 0F5A, 0FA2, 0FEA |
| `386E` | `386E:0EC0` | `OUT 3CEh` | GC 01h enable set/reset | 0F1A, 0F65, 0FAD, 0FF5 |
| `386E` | `386E:0EC0` | `OUT 3CEh` | GC 05h mode | 0F16, 0F61, 0FA9, 0FF1 |
| `386E` | `386E:0EC0` | `OUT 3CEh` | GC 08h bit mask | 0F1E, 0F69, 0FB1, 0FF9 |
| `386E` | `386E:1027` | `OUT 3C4h` | SEQ 02h map mask | 107A, 10C3, 110C, 1155 |
| `386E` | `386E:1027` | `OUT 3CEh` | GC 01h enable set/reset | 1085, 10CE, 1117, 1160 |
| `386E` | `386E:1027` | `OUT 3CEh` | GC 05h mode | 1081, 10CA, 1113, 115C |
| `386E` | `386E:1027` | `OUT 3CEh` | GC 08h bit mask | 1089, 10D2, 111B, 1164 |
| `386E` | `386E:1193` | `OUT 3C4h` | SEQ 02h map mask | 11E5, 122F, 1279, 12C3 |
| `386E` | `386E:1193` | `OUT 3CEh` | GC 01h enable set/reset | 11F0, 123A, 1284, 12CE |
| `386E` | `386E:1193` | `OUT 3CEh` | GC 05h mode | 11EC, 1236, 1280, 12CA |
| `386E` | `386E:1193` | `OUT 3CEh` | GC 08h bit mask | 11F4, 123E, 1288, 12D2 |
| `386E` | `386E:1302` | `OUT 3C4h` | SEQ 02h map mask | 1351, 1396, 13DB, 1420 |
| `386E` | `386E:1302` | `OUT 3CEh` | GC 01h enable set/reset | 135C, 13A1, 13E6, 142B |
| `386E` | `386E:1302` | `OUT 3CEh` | GC 05h mode | 1358, 139D, 13E2, 1427 |
| `386E` | `386E:1302` | `OUT 3CEh` | GC 08h bit mask | 1360, 13A5, 13EA, 142F |
| `386E` | `386E:145A` | `OUT 3C4h` | SEQ 02h map mask | 14A9, 14F4, 153C, 1584 |
| `386E` | `386E:145A` | `OUT 3CEh` | GC 01h enable set/reset | 14B4, 14FF, 1547, 158F |
| `386E` | `386E:145A` | `OUT 3CEh` | GC 05h mode | 14B0, 14FB, 1543, 158B |
| `386E` | `386E:145A` | `OUT 3CEh` | GC 08h bit mask | 14B8, 1503, 154B, 1593 |
| `386E` | `386E:15C1` | `OUT 3C4h` | SEQ 02h map mask | 1614, 165D, 16A6, 16EF |
| `386E` | `386E:15C1` | `OUT 3CEh` | GC 01h enable set/reset | 161F, 1668, 16B1, 16FA |
| `386E` | `386E:15C1` | `OUT 3CEh` | GC 05h mode | 161B, 1664, 16AD, 16F6 |
| `386E` | `386E:15C1` | `OUT 3CEh` | GC 08h bit mask | 1623, 166C, 16B5, 16FE |
| `386E` | `386E:172D` | `OUT 3C4h` | SEQ 02h map mask | 177F, 17C9, 1813, 185D |
| `386E` | `386E:172D` | `OUT 3CEh` | GC 01h enable set/reset | 178A, 17D4, 181E, 1868 |
| `386E` | `386E:172D` | `OUT 3CEh` | GC 05h mode | 1786, 17D0, 181A, 1864 |
| `386E` | `386E:172D` | `OUT 3CEh` | GC 08h bit mask | 178E, 17D8, 1822, 186C |
| `386E` | `386E:189C` | `OUT 3C4h` | SEQ 02h map mask | 18BA |
| `386E` | `386E:18D8` | `OUT 3C4h` | SEQ 02h map mask | 18ED |
| `386E` | `386E:191C` | `OUT 3C4h` | SEQ 02h map mask | 1939 |
| `386E` | `386E:191C` | `OUT 3C5h` | map mask data 0Fh (all planes) | 196D |
| `386E` | `386E:191C` | `OUT 3CEh` | GC 08h bit mask | 1933, 1985 |
| `386E` | `Gfx_SetLogicalWidth` (386E:1991) | `OUT 3D4h` | CRTC 13h offset | 199D |
| `386E` | `Gfx_SetSplitLine` (386E:19A0) | `IN DX(?)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write) | 19AC, 19DE |
| `386E` | `Gfx_SetSplitLine` (386E:19A0) | `IN DX(SUB DL)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write) | 19CE |
| `386E` | `Gfx_SetSplitLine` (386E:19A0) | `OUT DX(?)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write), index 07h | 19D9 |
| `386E` | `Gfx_SetSplitLine` (386E:19A0) | `OUT DX(?)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write), index 09h | 19DC, 19E9 |
| `386E` | `Gfx_SetSplitLine` (386E:19A0) | `OUT DX(SUB DL)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write), index 07h | 19CC |
| `386E` | `Gfx_SetSplitLine` (386E:19A0) | `OUT DX(SUB DL)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write), index 18h | 19C9 |
| `386E` | `Gfx_InitUnchained` (386E:19EF) | `OUT 3C4h` | SEQ 02h map mask | 1A4A |
| `386E` | `Gfx_InitUnchained` (386E:19EF) | `OUT 3C4h` | SEQ 04h memory mode (chain-4 off) | 1A31 |
| `386E` | `Gfx_InitUnchained` (386E:19EF) | `OUT 3C5h` | map mask data 0Fh (all planes) | 1A4E |
| `386E` | `Gfx_InitUnchained` (386E:19EF) | `OUT 3CEh` | GC 05h mode | 1A38 |
| `386E` | `Gfx_InitUnchained` (386E:19EF) | `OUT 3D4h` | CRTC 14h underline (dword off) | 1A3F |
| `386E` | `Gfx_InitUnchained` (386E:19EF) | `OUT 3D4h` | CRTC 17h mode control (byte mode) | 1A43 |
| `386E` | `Gfx_FlipPage` (386E:1A6C) | `OUT 3D4h` | CRTC 0Ch start address high | 1A83 |
| `386E` | `Gfx_FlipPage` (386E:1A6C) | `OUT 3D4h` | OUT DX,AX | 1A88 |
| `386E` | `386E:1A97` | `OUT 3C4h` | SEQ 02h map mask | 1ABF |
| `386E` | `386E:1A97` | `OUT 3CEh` | GC 01h enable set/reset | 1ACA |
| `386E` | `386E:1A97` | `OUT 3CEh` | GC 05h mode | 1AC6 |
| `386E` | `386E:1A97` | `OUT 3CEh` | GC 08h bit mask | 1ACE |

### BIOS video (INT 10h) (2 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `386E` | `Gfx_InitUnchained` (386E:19EF) | `INT 10h` | set video mode | 1A0A |
| `3A1C` (Crt) | `Crt_BiosVideo` (3A1C:0614) | `INT 10h` | set cursor position | 0618 |

### PIT (timer) (3 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `2C37` (WORX) | `WORX_SetTimerRate` (2C37:4A72) | `OUT 40h` | PIT ch0 divisor byte | 4A9D, 4AA1 |
| `2C37` (WORX) | `WORX_SetTimerRate` (2C37:4A72) | `OUT 43h` | PIT control word 34h | 4A9A |

### PIC (EOI) (2 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `0E8B` | `0E8B:065B` | `OUT 20h` | EOI | 06A1 |
| `2C37` (WORX) | `WORX_TimerIsr` (2C37:4558) | `OUT 20h` | EOI | 459F |

### Keyboard (5 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `0E8B` | `0E8B:065B` | `IN 60h` | scan code | 0666 |
| `3A1C` (Crt) | `Crt_CheckBreakFlag` (3A1C:014E) | `INT 16h` | key available? | 015D |
| `3A1C` (Crt) | `Crt_CheckBreakFlag` (3A1C:014E) | `INT 16h` | read key | 0163 |
| `3A1C` (Crt) | `Crt_KeyPressed` (3A1C:0308) | `INT 16h` | key available? | 0311 |
| `3A1C` (Crt) | `Crt_ReadKey` (3A1C:031A) | `INT 16h` | read key | 0328 |

### Joystick (8 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `1EBF` | `Inp_JoystickRead` (1EBF:0026) | `IN 201h` | trigger (OUT) / poll axes and buttons (IN) | 003E, 0056, 0093, 00B7, 0126, 014A |
| `1EBF` | `Inp_JoystickRead` (1EBF:0026) | `OUT 201h` | trigger (OUT) / poll axes and buttons (IN) | 0088, 011B |

### PC speaker (PIT ch2 + port 61h) (16 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `2C37` (WORX) | `2C37:3CDA` | `IN 61h` | speaker gate | 3D07 |
| `2C37` (WORX) | `2C37:3CDA` | `OUT 61h` | speaker gate | 3D0B |
| `2C37` (WORX) | `2C37:3F63` | `IN 61h` | speaker gate | 3F32, 3F58 |
| `2C37` (WORX) | `2C37:3F63` | `OUT 42h` | PIT ch2 divisor byte | 3F52, 3F56 |
| `2C37` (WORX) | `2C37:3F63` | `OUT 43h` | PIT control word B6h | 3F4A |
| `2C37` (WORX) | `2C37:3F63` | `OUT 61h` | speaker gate | 3F3A, 3F5C |
| `3A1C` (Crt) | `Crt_Sound` (3A1C:02D4) | `IN 61h` | speaker gate | 02E8 |
| `3A1C` (Crt) | `Crt_Sound` (3A1C:02D4) | `OUT 42h` | PIT ch2 divisor byte | 02F8, 02FC |
| `3A1C` (Crt) | `Crt_Sound` (3A1C:02D4) | `OUT 43h` | PIT control word B6h | 02F4 |
| `3A1C` (Crt) | `Crt_Sound` (3A1C:02D4) | `OUT 61h` | speaker gate | 02F0 |
| `3A1C` (Crt) | `Crt_NoSound` (3A1C:0301) | `IN 61h` | speaker gate | 0301 |
| `3A1C` (Crt) | `Crt_NoSound` (3A1C:0301) | `OUT 61h` | speaker gate | 0305 |

### DMA controller (Sound Blaster) (14 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `2C37` (WORX) | `2C37:4B49` | `OUT Ah` | OUT 0xa,AL (AL=05h) | 4B6A |
| `2C37` (WORX) | `2C37:4CF9` | `OUT 2h` | OUT 0x2,AL | 4D2F, 4D33 |
| `2C37` (WORX) | `2C37:4CF9` | `OUT 3h` | OUT 0x3,AL | 4D42 |
| `2C37` (WORX) | `2C37:4CF9` | `OUT 3h` | OUT 0x3,AL (AL=FFh) | 4D3E |
| `2C37` (WORX) | `2C37:4CF9` | `OUT 83h` | OUT 0x83,AL | 4D39 |
| `2C37` (WORX) | `2C37:4CF9` | `OUT Ah` | OUT 0xa,AL (AL=01h) | 4D46 |
| `2C37` (WORX) | `2C37:4CF9` | `OUT Ah` | OUT 0xa,AL (AL=05h) | 4CFE |
| `2C37` (WORX) | `2C37:4CF9` | `OUT Bh` | OUT 0xb,AL (AL=55h) | 4D29 |
| `2C37` (WORX) | `2C37:4CF9` | `OUT Ch` | OUT 0xc,AL | 4D1B |
| `2C37` (WORX) | `2C37:4DC4` | `IN 2h` | IN AL,0x2 | 4DCB, 4DCD |
| `2C37` (WORX) | `2C37:4DC4` | `OUT Ch` | OUT 0xc,AL | 4DC9 |
| `2C37` (WORX) | `2C37:631F` | `OUT Ah` | OUT 0xa,AL (AL=05h) | 636E |

### Sound Blaster DSP (base in a variable) (6 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `2C37` (WORX) | `2C37:3CAF` | `IN DX(?)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write) | 3CC4, 3CC5 |
| `2C37` (WORX) | `2C37:3CAF` | `OUT DX(?)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write) | 3CC0 |
| `2C37` (WORX) | `2C37:5D00` | `IN DX(?)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write) | 5D0E |
| `2C37` (WORX) | `2C37:5E47` | `IN DX(?)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write) | 5E61 |
| `2C37` (WORX) | `2C37:5E47` | `OUT DX(?)` | CRTC 3D4h/3D5h via DX = 3DAh - 6 (read-modify-write) | 5E69 |

### OPL2 / AdLib (2 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `2C37` (WORX) | `2C37:3CAF` | `IN 388h` | IN AL,DX | 3CBA |
| `2C37` (WORX) | `2C37:3CAF` | `OUT 388h` | OUT DX,AL | 3CB6 |

### MPU-401 MIDI (1 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `2C37` (WORX) | `2C37:5D00` | `OUT 330h` | OUT DX,AL | 5D18 |

### WORX driver API (INT 63h) (1 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `2C37` (WORX) | `WORX_CallDriver` (2C37:0000) | `INT 63h` | driver call | 0029 |

### DOS (INT 21h) (47 sites)

| Segment | Routine | Instruction | Register / purpose | Sites (offsets in the segment) |
|---|---|---|---|---|
| `2C37` (WORX) | `WORX_InstallInterrupts` (2C37:3285) | `INT 21h` | AH=25h set interrupt vector | 32BD, 3304 |
| `2C37` (WORX) | `WORX_InstallInterrupts` (2C37:3285) | `INT 21h` | AH=35h get interrupt vector | 32A8, 32EF |
| `2C37` (WORX) | `WORX_RemoveInterrupts` (2C37:336E) | `INT 21h` | AH=25h set interrupt vector | 339B, 33B1 |
| `2C37` (WORX) | `2C37:3536` | `INT 21h` | AH=3Fh read file/device | 35C1 |
| `2C37` (WORX) | `2C37:3536` | `INT 21h` | AH=42h seek | 357E |
| `2C37` (WORX) | `2C37:3763` | `INT 21h` | AH=3Eh close file | 3771 |
| `3A1C` (Crt) | `Crt_InitVideo` (3A1C:003B) | `INT 21h` | AH=25h set interrupt vector | 009F |
| `3A7E` (Dos) | `Dos_GetDate` (3A7E:0000) | `INT 21h` | AH=2Ah get date | 0005 |
| `3A7E` (Dos) | `Dos_SetDate` (3A7E:0022) | `INT 21h` | AH=2Bh set date | 0030 |
| `3A7E` (Dos) | `Dos_GetTime` (3A7E:0036) | `INT 21h` | AH=2Ch get time | 003B |
| `3A7E` (Dos) | `Dos_SetTime` (3A7E:005B) | `INT 21h` | AH=2Dh set time | 006C |
| `3A7E` (Dos) | `Dos_GetIntVec` (3A7E:0072) | `INT 21h` | AH=35h get interrupt vector | 007A |
| `3A7E` (Dos) | `Dos_SetIntVec` (3A7E:008A) | `INT 21h` | AH=25h set interrupt vector | 0096 |
| `3A7E` (Dos) | `Dos_Exec` (3A7E:009D) | `INT 21h` | AH=29h parse filename | 00FC, 010A |
| `3A7E` (Dos) | `Dos_Exec` (3A7E:009D) | `INT 21h` | AH=4Bh exec program | 0115 |
| `3A9A` (System) | `BP_SystemInit` (3A9A:0000) | `INT 21h` | AH=25h set interrupt vector | 006D, 0075, 007D, 0085 |
| `3A9A` (System) | `BP_SystemInit` (3A9A:0000) | `INT 21h` | AH=35h get interrupt vector | 0058 |
| `3A9A` (System) | `BP_Halt` (3A9A:0116) | `INT 21h` | AH=25h set interrupt vector | 01AF |
| `3A9A` (System) | `BP_Halt` (3A9A:0116) | `INT 21h` | AH=4Ch terminate | 01EE |
| `3A9A` (System) | `BP_PrintChar` (3A9A:0232) | `INT 21h` | AH=06h direct console output | 0236 |
| `3A9A` (System) | `BP_TextFileRead` (3A9A:066E) | `INT 21h` | AH=3Fh read file/device | 0682 |
| `3A9A` (System) | `BP_TextFileWrite` (3A9A:069E) | `INT 21h` | AH=40h write file/device | 06B4 |
| `3A9A` (System) | `3A9A:06C3` | `INT 21h` | AH=40h write file/device | 06D9 |
| `3A9A` (System) | `BP_TextFileClose` (3A9A:06E3) | `INT 21h` | AH=3Eh close file | 06F3 |
| `3A9A` (System) | `BP_RewriteFile` (3A9A:09D5) | `INT 21h` | AH set elsewhere | 0A12 |
| `3A9A` (System) | `BP_CloseFile` (3A9A:0A4D) | `INT 21h` | AH=3Eh close file | 0A62 |
| `3A9A` (System) | `3A9A:0A8D` | `INT 21h` | AH set elsewhere | 0AA4 |
| `3A9A` (System) | `BP_BlockWrite` (3A9A:0ABE) | `INT 21h` | AH set elsewhere | 0AE5 |
| `3A9A` (System) | `BP_SeekFile` (3A9A:0B1F) | `INT 21h` | AH=42h seek | 0B44 |
| `3A9A` (System) | `BP_ParamStr` (3A9A:0B4F) | `INT 21h` | AH=30h DOS version | 0B60 |
| `3A9A` (System) | `BP_TextOpenFunc` (3A9A:0F1B) | `INT 21h` | AH set elsewhere | 0F46 |
| `3A9A` (System) | `BP_TextOpenFunc` (3A9A:0F1B) | `INT 21h` | AH=44h IOCTL | 0F62 |
| `3A9A` (System) | `3A9A:0FA8` | `INT 21h` | AH=3Fh read file/device | 0FD5 |
| `3A9A` (System) | `3A9A:0FA8` | `INT 21h` | AH=40h write file/device | 0FFF |
| `3A9A` (System) | `3A9A:0FA8` | `INT 21h` | AH=42h seek | 0FB1, 0FC8, 0FF7 |
| `3A9A` (System) | `BP_FileQueryPos` (3A9A:1A1E) | `INT 21h` | AH=42h seek | 1A36, 1A44, 1A52 |
