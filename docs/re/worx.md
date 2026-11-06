# WORX Toolkit 2.1 in `file0001.exe` (M1-02)

WORX Toolkit 2.1 (Mystic Software, 1993) is the sound library linked into every TD2192 program. In `file0001.exe` it is code segment `2C37` (65B0h bytes). The segment holds two parts:

- `2C37:0000..040F`: **Pascal wrappers** that game code calls with `CALL FAR`. Each one fills a small parameter block in DGROUP and calls the driver.
- `2C37:0410..65AF`: the **assembler driver**. It keeps all of its state and tables CS-relative and is reached through a software interrupt it installs itself, **INT 63h**. A timer handler on INT 08h runs music and digital-sound streaming.

`file0000.exe` (intro) links the same driver shifted by +1Fh bytes. Apart from that shift, only 4 bytes differ, in code the music player does not use. `td2.exe` and `td3.exe` have a WORX segment of the same size (`26CB`, `22DA`).

This page documents the driver interface and what the game uses. The CMF player is specified in [spec/audio/cmf.md](../spec/audio/cmf.md), the cue table in [spec/audio/music-cues.md](../spec/audio/music-cues.md), and the effects policy in [spec/audio/sfx-policy.md](../spec/audio/sfx-policy.md). All addresses are `file0001:` image-relative.

## 1. Driver call convention (INT 63h)

`WORX_CallDriver(fn: Byte): Word` (`2C37:0000`) loads the registers and issues `INT 63h`:

| Register in | Source | Register out | Copied to |
|---|---|---|---|
| AH | `fn` | AX | DS:394A (also the function result) |
| AL | DS:3950 (byte) | BX | DS:394E |
| BX | DS:3952 | DX | DS:394C |
| DX | DS:3954 (segment of a far pointer) | DI | DS:3956 |
| DI | DS:3956 (offset of a far pointer) | | |

The handler (`2C37:34EE`) increments a busy counter (CS:42DC), saves the registers, sets DS = CS and ES = DX, and calls the near routine whose offset is at `CS:2428 + 2·AH`. It returns that routine's AX and DX.

### 1.1 Function table (`2C37:2428`, 80 entries)

"Used" means a Pascal wrapper in `file0001` calls it. Semantics are LIKELY unless marked; unused entries were not studied.

| fn | Handler | Used | Meaning |
|---:|---|:---:|---|
| 01h | `4B95` | yes | SB init: set up the DMA buffer inside the driver segment, reset/detect the DSP, hook the SB IRQ, speaker on (`D1h`), SB Pro mixer master/voice/FM to FFh when the mixer answers. Returns the IRQ number, or FFFFh if there is no card |
| 02h | `4C62` | | set SB base port (BX) and IRQ |
| 03h | `3496` | | — |
| 04h | `5DD0` | yes | SB shutdown: stop digital sound, restore the IRQ vector and mask |
| 05h | `6221` | | — |
| 06h | `62D5` | yes | **Play VOC from memory** (DX:DI). Stops the digital sound that is playing first, then starts the new one ([voc §2.1](../spec/audio/voc-and-adpcm.md#21-how-worx-plays-a-voc-for-reference)). CONFIRMED |
| 07h | `4B49` | | stop digital sound (DSP `D0h`, DMA mask) |
| 08h | `49ED` | yes | digital sound playing? (bit 0 of CS:31F2; file0000 CS:3211). Set when a VOC starts, cleared by a stop or when the streaming code next finds the end of the data, so it stays set 0.1..0.38 s after the last sample at preset P1 (measured: [flow/intro](../spec/flow/intro.md) §3.6, C-0081; U-410) |
| 09h–0Bh | `34A2`, `34C9`, `34E3` | | — |
| 0Ch | `466D` | | start a song of another format (the loop restart calls it when the song type is not CMF; HYPOTHESIS: Standard MIDI File) |
| 0Dh | `427F` | yes | **Stop music** ([cmf §4.9](../spec/audio/cmf.md#49-stop-worx-function-0dh-and-loop-control)). CONFIRMED |
| 0Eh | `49E5` | | music playing? (CS:316A; file0000 CS:3189). Cleared by a stop or an end of track with looping off; a loop restart sets it again inside the same interrupt, so a caller never reads 0 at a loop (U-370; [flow/intro](../spec/flow/intro.md) §4.6). Used only by the intro |
| 0Fh | `3B9A` | | copy one instrument from an SBI-format record in memory (checks the `SB` signature, skips 24h bytes) into bank entry BX |
| 10h, 11h | `41D3`, `41DB` | | — |
| 12h | `4233` | | program change: channel AL, program BL (same code as a CMF `Cx` event) |
| 13h | `340C` | | set the device flags CS:31C4 (§3) |
| 14h–16h | `370A`, `3AEA`, `3411` | | — |
| 17h | `35D2` | yes | open a source: a file by name, or memory (BX = size). Returns the size in DX:AX, −1 on failure |
| 18h | `3536` | yes | read BX bytes from the open source into DX:DI |
| 19h | `3763` | | close the file |
| 1Ah–1Ch | `3783`, `38C9`, `37B7` | | — |
| 1Dh | `4263` | yes | **set music loop flag** (BL → CS:3069). CONFIRMED |
| 1Eh | `4791` | yes | **Play CMF from memory** (DX:DI) ([cmf §4.3](../spec/audio/cmf.md#43-song-start-worx-function-1eh-also-used-for-the-loop-restart)). CONFIRMED |
| 1Fh | `49FA` | | resume music (CS:316A := 1) |
| 20h | `3B37` | | SB Pro mixer register 22h (master) := BL |
| 21h | `3B79` | yes | SB Pro mixer register 04h (voice volume) := BL (high nibble left, low nibble right). Needs an SB Pro mixer (CS:31FA ≥ 1). CONFIRMED |
| 22h | `3B58` | yes | SB Pro mixer register 26h (FM volume) := BL. CONFIRMED |
| 23h | `3425` | yes | AdLib detection (OPL timer test) |
| 24h, 25h | `3367`, `33D7` | | — |
| 26h | `327E` | yes | mark the driver initialised (CS:3014 := 1) |
| 29h–2Ch | `45FE`, `45BB`, `45D5`, `45EF` | | — |
| 2Dh | `49F5` | | music beat counter (CS:305E) |
| 2Eh | `426F` | | PIT back to 18.2 Hz and lock further rate changes |
| 2Fh | `487E` | | music tick (the ISR's routine) |
| 30h | `4878` | | — |
| 31h–36h | `50E9`, `5A7D`, `5AF8`, `5A81`, `507D`, `5A6C` | | multi-voice digital mixer (not used: device flag 20h is never set) |
| 37h–3Dh | `4C6E`, `5D2E`, `5D7B`, `6177`, `60FE`, `5FD5`, `60D8` | | SB/DSP helpers |
| 3Eh | `42DF` | yes | **enable timer streaming** (BL → CS:42DE): the timer's second-level work refills the VOC DMA buffer. CONFIRMED |
| 3Fh | `606B` | | set the DSP command index (CS:4CEC) |
| 40h–4Fh | `399F`, `5B9A`, `61FA`, `38C3`, `5069`, `6021`, `3CA2`, `5073`, `2620`, `2751`, `2610`, `25E4`, `25F4`, `3D43`, `5951`, `5E47` | | — (`5E47` is the raw DSP write) |
| 00h, 27h, 28h | 0 | | none |

## 2. Pascal wrappers (`2C37:0000..040F`) and game use

Names are ours (added to [symbols/file0001.csv](symbols/file0001.csv)). "Calls" counts call sites in `file0001`.

| Wrapper | Name | Signature (Pascal order) | fn | Calls | Game use |
|---|---|---|---:|---:|---|
| `0000` | `WORX_CallDriver` | `(Fn: Byte): Word` | any | 14 | — |
| `0055` | `WORX_Shutdown` | `` | — | 4 | removes INT 08h/63h (`2C37:336E`): stop VOC, stop music, PIT back to 18.2 Hz, close the file. Exit paths in `0000:3381` and `Main_Entry` |
| `0066` | `WORX_ReadToBuffer` | `(Seg, Ofs, Size: Word): Word` | 18h | 2 | inside the loaders |
| `009F` | `WORX_FileSize` | `(Name: String): LongInt` | 17h | 2 | inside the loaders |
| `010A` | `WORX_LoadSfx` | `(Name: String): Pointer` | 17h, 18h | 10 | loads a file into a new heap block (`GetMem(size)`, low word only, so < 64 KB); size → DS:3948. Used for the 10 VOCs ([sfx-policy §3](../spec/audio/sfx-policy.md#3-data)) |
| `0273` | `WORX_LoadMusic` | `(Name: String): Pointer` | 17h, 18h | 14 | same, size → DS:3946. Used by `Snd_PlayMusic` for the 14 CMFs |
| `01AB` | `WORX_SetVoiceVolume` | `(Left, Right: Byte)` | 21h | 1 | per-effect panning (`1E2A:0223`) |
| `01E2` | `WORX_SetFmVolume` | `(Left, Right: Byte)` | 22h | 1 | once at start-up: (10, 10) |
| `0219` | `WORX_DetectAdLib` | `: Word` | 23h | 2 | `/config` auto-detection |
| `0238` | `WORX_SbShutdown` | `` | 04h | 4 | exit paths |
| `0254` | `WORX_SbInit` | `: Word` | 01h | 1 | start-up; result → DS:0502 (−1 = no SB) |
| `031E` | `WORX_PlayVoc` | `(P: Pointer): Word` | 06h | 12 | `Snd_PlayEffect` |
| `034B` | `WORX_PlayCmf` | `(P: Pointer): Word` | 1Eh | 1 | `Snd_PlayMusic` |
| `0378` | `WORX_SetMusicLoop` | `(On: Word)` | 1Dh | 8 | 0 before and 1 after each non-repeating death theme |
| `039C` | `WORX_Init` | `: Word` | 26h | 1 | start-up: installs INT 63h and INT 08h (PIT divisor 0952h, 500 Hz) |
| `03BD` | `WORX_StopMusic` | `` | 0Dh | 6 | `Snd_PlayMusic`, flight start/end, one story scene, exit |
| `03D3` | `WORX_VocPlaying` | `: Word` | 08h | 2 | in flight (priority reset) and at mission end (wait for silence) |
| `03F2` | `WORX_SetTimerStreaming` | `(On: Word)` | 3Eh | 1 | start-up, 1 when an SB was found |

Start-up order in `Main_Entry` (`0000:47C2..4812`), skipped entirely when the first parameter is `/nosound` or `/pc` (flag DS:13DC): `WORX_Init`, `WORX_SbInit` → DS:0502, `WORX_SetTimerStreaming(1)` if an SB was found, `WORX_SetFmVolume(10, 10)`. The sound settings (DS:12C6 music, DS:12C7 effects, DS:12C8 controller) come from `td.cfg` or the switches; see [sfx-policy §2](../spec/audio/sfx-policy.md#2-definitions-and-units).

Game-side sound routines (our names):

| Address | Name | What it does |
|---|---|---|
| `0E8B:07ED` | `Snd_PlayMusic(N: Byte)` | only if DS:12C6 = 2: `WORX_StopMusic`, free the previous song, load song N (table in [music-cues §3](../spec/audio/music-cues.md#3-data)), `WORX_PlayCmf`. CONFIRMED |
| `1E2A:006A` | `Snd_PlayEffect(Id, SrcX, SrcY, ViewX, ViewY, Device: Byte)` | [sfx-policy §4](../spec/audio/sfx-policy.md#4-behaviour). CONFIRMED |
| `1E2A:06C6` | `Snd_LoadEffects(Device: Byte)` | Device 3: hook INT 1Ch (speaker sequencer `1E2A:0000`). Device 2: load the 10 VOCs |
| `1E2A:0810` | `Snd_FreeEffects(Device: Byte)` | the reverse, at mission end |

## 3. Driver state worth knowing

| CS offset | Meaning |
|---|---|
| 31C4 | Device flags, initial 0Ch. Bit 1: music on the PC speaker; bit 2: FM (OPL2); bit 4: MPU-401 MIDI out; bit 5: multi-voice digital mixer. The timer runs the music tick when `flags and 16h ≠ 0` |
| 3069 | Music loop flag (initial 1) |
| 316A | Music playing |
| 3062 / 305C / 305E | Header 0x0A (ticks per quarter) / tick-within-beat / beat counter |
| 3051 | Current PIT divisor |
| 31BC / 31BE | Fast-tick counter / chain interval N = FFFFh div divisor ([timing §4.2](../spec/core/timing.md#42-where-the-bios-tick-comes-from)) |
| 291B | Instrument bank, 11 bytes per program (a default bank is preloaded) |
| 2F9D / 2FAF / 2FBF / 2FCF / 2FD8 / 2F8D | Voice owner+program words / channel programs / channel volumes / voice notes / voice carrier levels / channel transposes |
| 31D1, 2F55 | BDh shadow, GM-drum flag |
| 31F2 | Digital sound active |
| 31F8, 31FA | SB base port; SB Pro mixer present |
| 4CEB, 4CEC, 4CED | VOC block state, codec/DSP-command index, time constant |
| 42DE | Timer streaming enabled |

## 4. Timer interrupt

`WORX_TimerIsr` (`2C37:4558`) runs at the PIT rate. In order: if `flags and 16h ≠ 0` it runs the music tick (`2C37:487E`, [cmf §4.4](../spec/audio/cmf.md#44-each-tick)). Then it counts fast ticks and chains to the BIOS handler every N ticks. On the other ticks it sends EOI and runs the second-level work (`2C37:44EF`) unless the driver is busy. The second-level work does mixer streaming (flag 20h; not used) and, when timer streaming is on, refills the VOC DMA buffer (`2C37:61FA`). Rates are in [timing §4.2](../spec/core/timing.md#42-where-the-bios-tick-comes-from).

## 5. Quirks found

| Quirk | Where | Effect |
|---|---|---|
| "Clear all OPL registers" writes register 00h with values 0..255 (register and value swapped) | `2C37:32D4`, `2C37:3BCC` | OPL registers are never actually cleared; harmless on hardware |
| Song start writes `00=00` | `2C37:41E8` | harmless |
| Stop passes its function number 0Dh as the channel of an all-notes-off | `2C37:427F` → `3CDA` | only channel-13 voices get the extra key-off |
| Program change compares the channel with B0h instead of 0Bh | `2C37:4247` | percussion program changes are not skipped (no audible effect) |
| Controller 68h stores the "up" flag in the value and then subtracts | `2C37:3E4A`, `4102..4128` | transpose-up lowers the pitch (unused by our songs) |
| Status Ax (aftertouch) is handled as a controller | `2C37:3EB3` | would misread notes 07h/67h–69h/7Bh as controllers (no Ax events in our songs) |

## 6. Provenance and method

Static reading of the listing, made in a private Ghidra copy (`re-work\ghidra-re4`), where the jump-table handlers were disassembled. The CMF player was also executed on an 8086 interpreter. Its OPL output matches the DOSBox-X capture C-0001 write for write ([cmf §6.2](../spec/audio/cmf.md#62-capture-checks)), which confirms the music-related rows above. The VOC and mixer rows are static only.

| Claim | Address | Confidence |
|---|---|---|
| INT 63h dispatch through `CS:2428` | `2C37:34EE..3530` | CONFIRMED |
| Wrapper register convention | `2C37:0000..0054` | CONFIRMED |
| Wrapper → fn mapping | `2C37:0055..040F` | CONFIRMED |
| Music functions 0Dh, 1Dh, 1Eh | see cmf.md | CONFIRMED (emulated + capture) |
| VOC functions 06h, 08h, 3Eh; DSP command table | `2C37:62D5`, `49ED`, `42DF`, `31DC` | CONFIRMED (static) |
| Other rows of §1.1 | handler entry code only | LIKELY or unknown, as marked |
