# Creative Music File playback

| | |
|---|---|
| **Spec ID** | `audio/cmf` |
| **Revision** | 2 |
| **Status** | Draft |
| **Fidelity tier** | T1 for the OPL register-write stream (it is fully determined, and the port can reproduce it write for write); T2 for the resulting audio |
| **Confidence** | CONFIRMED. Three lines of evidence: (1) static reading of the WORX player (`file0001:2C37`); (2) the WORX code run on an 8086 interpreter reproduces all 4,364 OPL writes of the DOSBox-X capture C-0001 in order and value; (3) an independent model written from this spec reproduces the emulated driver exactly on all 17 shipped songs (146,595 writes, same tick, register and value) |
| **Tasks** | M3-04, M3-05 |
| **Unknowns** | U-007 (resolved), U-034 (resolved) |

## 1. Summary

Music is 17 Creative Music Files (CMF v1.01) for a Yamaha YM3812 (OPL2), played by the WORX Toolkit 2.1 driver linked into every game program ([worx.md](../../re/worx.md)). A CMF is a header, a bank of OPL instrument definitions and one MIDI-like event track. WORX does **not** behave like Creative's SBFMDRV. It allocates voices dynamically, ignores note velocity, scales the carrier level by the channel volume (default 127), shifts every melodic note down one octave, toggles percussion bits instead of setting them, uses a built-in drum kit in rhythm mode, and loads an instrument only when a note needs it. This revision gives the exact rules. A port that follows §4 produces the same OPL register writes as the original, tick for tick.

## 2. Definitions and units

| Term | Meaning |
|---|---|
| Tick | One music step. WORX runs one step per PIT interrupt, with the PIT divisor set from the header clock (§4.1) |
| Voice | One of the nine OPL2 two-operator channels 0..8 |
| Channel | MIDI channel 0..15 of an event (low nibble of the status byte) |
| Rhythm mode | OPL2 percussion mode (register BDh bit 5). Voices 6..8 then belong to the drums and only voices 0..5 are melodic |
| Program | Instrument number from a program-change event (0..127). It indexes the song's instrument bank |
| Operator offsets | Modulator of voice v: `MOD[v] = 00 01 02 08 09 0A 10 11 12`; carrier: `CAR[v] = 03 04 05 0B 0C 0D 13 14 15` |
| Write | One (register, value) pair sent to the OPL (index port 388h, data port 389h) |

## 3. Data

### 3.1 Header (v1.01)

| Offset | Type | Field | Used by WORX |
|---:|---|---|---|
| 0x00 | 4 bytes | `CTMF` | checked; playback does not start otherwise |
| 0x04 | u16 | Version | no |
| 0x06 | u16 | Instrument block offset | yes |
| 0x08 | u16 | Music data offset | yes |
| 0x0A | u16 | Ticks per quarter note (23..192 in our files) | only for the tempo meta event and a beat counter (§4.8) |
| 0x0C | u16 | **Ticks per second** (96 in every file) | **yes: the playback clock** (U-007) |
| 0x0E | 3 × u16 | Title, composer, remarks offsets | no |
| 0x14 | 16 × u8 | Channel-in-use flags | no |
| 0x24 | u16 | Instrument count | yes |
| 0x26 | u16 | Basic tempo | no |

### 3.2 Instrument record

WORX copies the first 11 of the 16 bytes of each record into its bank, at index 0, 1, 2 … in file order.

| Byte | Register written (voice v) |
|---:|---|
| 0, 1 | 20h + MOD[v], 20h + CAR[v] |
| 2, 3 | 40h + MOD[v], 40h + CAR[v] (byte 3 is also remembered as the voice's *carrier level*, §4.5) |
| 4, 5 | 60h + MOD[v], 60h + CAR[v] |
| 6, 7 | 80h + MOD[v], 80h + CAR[v] |
| 8, 9 | E0h + MOD[v], E0h + CAR[v] |
| 10 | C0h + v |
| 11–15 | ignored |

The bank is not cleared between songs: a new song overwrites entries 0..count−1, and higher entries keep older contents. Before the first song, entry 0 holds a WORX built-in instrument: `01 01 4F 12 F1 D3 50 7C 00 00 06` (bytes 0–10, from the driver's initialised data at `file0001:2C37:291B`; the area holds a full default bank, presumably for MIDI-file playback).

### 3.3 F-number table (`2C37:2F56`)

Index = semitone 0..11, starting at C#:

`16B 181 198 1B0 1CA 1E5 202 220 241 263 287 2AE` (hex)

### 3.4 Constants

| Constant | Value | Provenance |
|---|---|---|
| Global note offset | −12 (byte F4h, added with 8-bit wrap) | `2C37:31AB` |
| Default channel volume | 127 for all 16 channels; **not** reset when a song starts | `2C37:2FBF` |
| Transpose step | 22 F-number units per 128/128 semitone, for every semitone | `2C37:2F80` (13 × 16h) |
| Data bytes per status nibble | 8x, 9x, Ax, Bx, Ex: 2; Cx, Dx: 1; Fx: 0 | `2C37:31AC` |
| PIT clock constant | 1234DCh = 1,193,180 | `2C37:47F2` |
| Default loop flag | 1 (loop) | `2C37:3069` |

### 3.5 Rhythm-mode initialisation table (`2C37:2EC3`)

Written, in this order, when controller 67h turns rhythm mode on (§4.7). Register=value:

```text
BD=E0 08=00 A6=38 B6=09 50=0B C6=00 70=A8 90=4C 30=00 F0=00 53=04 73=D6 93=4F 33=00 F3=00
BD=E0 08=00 BD=E0 08=00 A7=03 B7=0A 51=0D C7=01 71=FA 91=8C 31=01 F1=00 BD=E0 08=00
BD=E0 08=00 A8=57 B8=09 52=00 C8=01 72=F7 92=B5 32=04 F2=00 BD=E0 08=00
BD=E0 08=00 54=07 74=F8 94=47 34=0C F4=00 BD=E0 08=00
BD=E0 08=00 55=0D 75=F5 95=A5 35=01 F5=00 BD=E0 08=00
```

These set the drum pitches (voices 6–8: A6/B6, A7/B7, A8/B8, with the key-on bits clear) and the drum operator patches. **The song's own instruments are never used for drums.**

## 4. Behaviour

### 4.1 Clock

When a song starts, WORX sets PIT channel 0 to divisor `1193180 div clock`, where `clock` = header word 0x0C. For our files that is 12428, so one tick lasts 1/96.0067 s (10.4159 ms). The same interrupt also drives the BIOS tick; see [timing §4.2](../core/timing.md#42-where-the-bios-tick-comes-from). Header word 0x0A does not affect the tick rate.

### 4.2 Player state

```text
bank[0..]          11-byte instruments (§3.2)
bd                 shadow of register BDh (C0h or E0h plus drum bits)
gm_drums           flag set by controller 67h = 1, cleared by 67h = 0; NOT reset by song start
loop               set by the game through WORX function 1Dh (§4.9)
playing
chan_program[16], chan_transpose[16], chan_volume[16]
voice_owner[9]     channel that owns the voice, or FREE (FFh)
voice_program[9]   program loaded into the voice, or FREE
voice_note[9]      note number sounding on the voice, 0 = silent
voice_level[9]     carrier level byte from the last instrument written to the voice
track position, running status, delay counter
```

### 4.3 Song start (WORX function 1Eh, also used for the loop restart)

1. Write `00=00`, set `bd := C0h`, write `BD=C0`.
2. For v = 0..8: write instrument `bank[0]` to voice v (§4.6). **Note:** this happens before the new bank is loaded, so it uses entry 0 of the previous song (or the built-in entry before the first song).
3. `chan_program[*] := 0`, `chan_transpose[*] := 0`, `voice_note[*] := 0`, `voice_owner[*] := voice_program[*] := FREE`. Channel volumes and `gm_drums` are kept.
4. Set the PIT divisor from header 0x0C (§4.1).
5. Copy the instrument records into `bank` (§3.2).
6. Track position := music offset. `delay` := the first delta time (variable-length quantity). `playing := true`.

If the file does not start with `CTMF`, steps 1–3 still happen and the rest is skipped (the game never passes such a file; what the player does next was not traced).

### 4.4 Each tick

```text
tick():
  if not playing: return
  loop:
    if delay > 0: delay := delay − 1; return
    read the next event (§4.4.1) and execute it
    if the event ended the song: return
    delay := next delta time (variable-length quantity)
```

So an event with delta `d` after an event at tick T is executed at tick T + d (d = 0: same tick). The first event of a song runs at tick `d0 + 1`, counting the first tick after the start as tick 1. After a loop restart (§4.8) it runs `d0` ticks after the end-of-track tick.

#### 4.4.1 Event decoding

- Byte FFh: meta event. Read the type byte. `2Fh` = end of track (§4.8). `51h` = set tempo (§4.8). Any other type: skip a one-byte length and that many bytes.
- Byte with bit 7 set: new running status; consume it.
- Otherwise the running status applies, and the byte is the first data byte.
- Read 0, 1 or 2 data bytes as given by §3.4 (status F0h reads none: SysEx is not supported).

#### 4.4.2 Dispatch

| Status | Action |
|---|---|
| 8x, 9x | note event (§4.5). Unless rhythm mode is on and x ≥ 11, add −12 to the note number first (8-bit wrap) |
| Ax | handled exactly like Bx: the note byte is read as a controller number (a bug; our files have no Ax events) |
| Bx | controller (§4.7) |
| Cx | `chan_program[x] := data1` |
| Dx, Ex | ignored (no channel pressure, **no pitch bend**) |

### 4.5 Notes

```text
note_event(ch, status, note, vel):
  nv := 6 if bd has bit 20h else 9                       # melodic voices
  if rhythm mode and ch ≥ 11:                             # CMF percussion
      bd := bd xor (10h >> (ch − 11))                     # 11 BD, 12 SD, 13 TT, 14 CY, 15 HH
      write BD=bd;  return                                # same for note-on and note-off (toggle)
  if status is 8x, or vel = 0:                            # note off
      v := lowest voice < nv with voice_owner = ch and voice_note = note
      if found: output(v, ch, note, key_on = false); voice_note[v] := 0
      return
  v := allocate(ch, nv);  if none: return                 # note dropped
  if voice_program[v] ≠ chan_program[ch]:
      voice_owner[v] := ch;  voice_program[v] := chan_program[ch]
      write instrument bank[chan_program[ch]] to v (§4.6)
  output(v, ch, note, key_on = true)

allocate(ch, nv):
  1. lowest v < nv with voice_owner = ch and voice_note = 0           (own silent voice)
  2. else lowest v < nv with voice_owner = FREE and voice_note = 0    (never used since start)
  3. else lowest v < nv with voice_note = 0: set its owner and program to FREE, use it
  4. else none: the note is not played (no voice stealing from sounding notes)

output(v, ch, note, key_on):
  voice_note[v] := note
  k := (note − 1) mod 256;  block := k div 12;  f := FNUM[k mod 12]
  t := chan_transpose[ch]
  if t ≠ 0:
      a := floor(t × 22 / 128)                            # t includes the 80h flag of controller 68h
      f := f + a if t = 80h else f − a
  write A0+v = f mod 256
  write B0+v = ((block × 4) mod 256) or (20h if key_on and not (gm_drums and ch ≥ 9)) or ((f div 256) mod 4)
  loud  := 63 − (voice_level[v] mod 64)
  level := 63 − ((floor(loud × chan_volume[ch] / 256) × 2) mod 256)
  write 40h + CAR[v] = level                              # KSL bits of the instrument are dropped
```

Consequences:

- **Velocity is ignored** (U-034). The carrier level depends only on the instrument's carrier total level and the channel volume. With the default volume 127, `level = 63 − 2·floor((63 − TL)·127/256)`: TL 0 → 1, TL 16 → 17, TL 63 → 63. The note-off write repeats the same level.
- **Pitch:** with the −12 offset, note n plays `FNUM[(n − 13) mod 12]` in block `(n − 13) div 12`. That is the coordinator's capture rule `T[(n−1) mod 12]`, block `(n−1) div 12 − 1`, for n ≥ 13. Notes below 13 wrap to huge blocks, which corrupt the B0 bits (vector V7). Blocks ≥ 8 do the same.
- The modulator level is never rescaled.
- The note-off search does not care which voice the note started on. If the same note number is sounding twice on one channel, the lowest-numbered voice is released first.
- A note that arrives while all nv voices are sounding is lost.

### 4.6 Instrument write

For voice v and instrument b[0..10]: `20+MOD=b0, 20+CAR=b1, 40+MOD=b2, 40+CAR=b3, 60+MOD=b4, 60+CAR=b5, 80+MOD=b6, 80+CAR=b7, E0+MOD=b8, E0+CAR=b9, C0+v=b10`, in that order, and `voice_level[v] := b3`. Register 01h (wave-select enable) is set to 20h once at driver start-up, so E0 writes take effect. A program number beyond the song's bank reads stale bank entries (does not occur in our files).

### 4.7 Controllers (Bx)

| Controller | Action |
|---|---|
| 07h volume | `chan_volume[x] := value`. Applies from the next note output on that channel. (When `gm_drums` is set and x = 9, it also rescales the drum operator levels; not used by our files) |
| 67h rhythm | value 0: `gm_drums := false`, `bd := C0h`, write `BD=C0`, voices 6–8 owner/program := FREE. Value 1: `gm_drums := true`, `bd := E0h`, write `BD=E0`, voices 6–8 owner := 10h (unusable), program := FREE, write the §3.5 table, then `BD=E0`, `08=00`. Other values: ignored |
| 68h, 69h transpose | `chan_transpose[x] := value or (80h if 68h)`. Applies from the next output (note-on or note-off) on that channel |
| 7Bh all notes off | for every voice owned by x: write `A0+v=00`, `B0+v=00`, `voice_note[v] := 0` |
| anything else (including 63h depth, 66h marker) | ignored. BDh keeps the depth bits C0h permanently |

Controller 69h value v lowers the F-number by `floor(v·22/128)` units (½ semitone = 11 units). Because of the flag bit, 68h lowers the pitch by `floor((v+128)·22/128)` when v > 0, and raises it by 22 only when v = 0. That looks like a WORX bug, and our files never use 68h.

**GM drum channel:** while `gm_drums` is set and rhythm mode is on, channel 9 notes (on or off) toggle a drum bit chosen by the note number (table `2C37:2E9B`, notes 35..74) and are not transposed. Our files do not use channel 9.

### 4.8 End of track, loop and tempo

- `FF 2F`: if `loop`, run the song start (§4.3) at once, in the same tick, and continue with the delay check of the new first event. Notes still sounding are **not** keyed off (their `voice_note` is cleared, so they may hang until the voice is reused). If not `loop`: `playing := false` and nothing is written.
- `FF 51 03 tt tt tt` (tempo, µs per quarter note, big-endian): new PIT divisor = `10 × floor(floor(tempo / ticks_per_quarter) × 7795 / 65536)`, where `ticks_per_quarter` = header 0x0A. None of our files contains this event.
- A beat counter (ticks / header 0x0A) is kept for WORX function 2Dh. The game does not read it.

### 4.9 Stop (WORX function 0Dh) and loop control

`stop` (called by the game before every new song, [music-cues](music-cues.md)):

1. For every voice whose owner is channel 13, write `A0+v=00`, `B0+v=00` and clear its note. (WORX passes its function number 0Dh as the channel of an "all notes off": a bug with almost no effect.)
2. `playing := false`.
3. If `bd and E0h = E0h`: write `BD=C0`, `08=00`, then `A0..A5 = 00`, then `B0..B5 = 00`. Otherwise write `A0..A8 = 00`, then `B0..B8 = 00`. `bd` itself is not changed.

Function 1Dh sets `loop` (game: 0 before the non-repeating death theme, 1 after).

### 4.10 What a port must do

- Run the event interpreter at `1193180 / (1193180 div clock)` Hz (96.0067 Hz for every shipped song), independent of the game tick.
- Reproduce §4.3–§4.9 exactly: same writes in the same order. Feed them to the OPL emulator (`td-opl`).
- Keep the cross-song state (bank leftovers, channel volumes, `gm_drums`) for the whole session, as the original does.

## 5. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| Velocity ignored | All notes on a channel are equally loud | Preserve (Classic). Enhanced may not "fix" it either: the music was mixed for this |
| Notes < 13 or block ≥ 8 | Corrupt B0 value | Preserve (does not occur in our files) |
| Toggle drums | A second note-on without a note-off silences the drum | Preserve |
| Loop restart without key-off | Notes held at the end of the track can hang | Preserve |
| Init writes use the previous song's instrument 0 | Inaudible (no key-on) | Preserve for write-exact parity |
| AH=0Dh all-notes-off | Only channel-13 voices | Preserve |
| 68h transpose sign | Lowers instead of raising | Preserve (unused) |
| No voice stealing | Notes lost when all voices are busy | Preserve |

## 6. Test vectors

### 6.1 Synthetic songs

Our own test songs (two made-up instruments: I0 = `01 11 4F 00 F1 F2 53 74 00 00 06`, I1 = `21 31 8A 90 E2 D3 15 26 01 02 0B`), played on the emulated original driver; the model gives the same output. Writes after the song-start block, as `reg=val`.

| # | Events (delta: event) | Expected writes |
|---|---|---|
| V1 | 0: B0 67 00; 0: C0 01; 0: 90 45 7F; 10: 90 45 00 | tick 1: `BD=C0`, I1 to voice 0 (`20=21 23=31 40=8A 43=90 60=E2 63=D3 80=15 83=26 E0=01 E3=02 C0=0B`), `A0=41 B0=32 43=11`. Tick 11: `A0=41 B0=12 43=11` |
| V2 | as V1 with velocity 14h | identical to V1 (velocity ignored) |
| V3 | 0: B0 67 00; 0: B0 07 40; 0: C0 01; 0: 90 45 7F | as V1 tick 1 but `43=29` |
| V4 | 0: B0 67 00; 0: B0 69 40; 0: 90 3C 64 | I0 to voice 0, `A0=A3 B0=2E 43=01` (2AEh − 11) |
| V5 | 0: B0 67 00; 0: 90 3C 64; 1: 90 40 64; 1: C1 01; 0: 91 43 64; 1: 90 3C 00; 1: 92 48 64; 1: 90 3E 64 | t1 voice 0 (I0) `A0=AE B0=2E 43=01`; t2 voice 1 (I0) `A1=B0 B1=31 44=01`; t3 voice 2 (I1) `A2=02 B2=32 45=11`; t4 `A0=AE B0=0E 43=01`; t5 ch 2 takes **voice 3** (I0) `A3=AE B3=32 4B=01`; t6 ch 0 reuses voice 0, no instrument write: `A0=81 B0=31 43=01` |
| V6 | 0: B0 67 01; 2: 9B 24 64; 2: 9B 24 00; 2: 9C 26 64; 2: 9C 26 64; 2: 9F 2A 64 | t1: `BD=E0` + §3.5 table + `BD=E0 08=00`; t3 `BD=F0`; t5 `BD=E0`; t7 `BD=E8`; t9 `BD=E0` (toggle); t11 `BD=E1` |
| V7 | 0: B0 67 00; 0: 90 01 64; 0: 91 0D 64; 0: 92 6C 64; 0: 93 7F 64 | `A0=CA B0=71`, `A1=6B B1=21`, `A2=AE B2=3E`, `A3=02 B3=26` (each after its I0 write, with level 01) |
| V8 | loop on; 3: B0 67 00; 0: 90 3C 64; 2: 90 3C 00; 0: FF 2F 00 | t4 note on; t6 note off, then the restart block (`00=00 BD=C0` + I0 to voices 0..8) in the **same tick**; t9 `BD=C0`, note on again; t11 off + restart; t14 … (period 5 ticks; the first pass took 6) |

### 6.2 Capture checks

| # | Setup | Expected | Evidence |
|---|---|---|---|
| C1 | `intro1.cmf`, `intro.cmf`, `intro2.cmf` as played by `file0000` (loop off, intro1 stopped after 1937 ticks, intro2 started 2 ticks after intro ended, stop at tick 12634) | The 4,364 writes in C-0001's DRO, from the first key-on to the stop, in order and value (the DRO drops writes that do not change a register; apply the same filter) | C-0001 `file0000_000.dro` |
| C2 | Any song, tick k | Register writes at 10.4159 ms × k from the start; capture timing agrees within −18..+11 ms | C-0001 |
| C3 | Durations at 96 ticks/s | climax 53.8 s, death 28.8 s, hangar 48.1 s, intro 84.0 s, td01 116.0 s, vulture 100.0 s (sum of delta times / 96, plus one tick) | data survey |

## 7. Open questions

- [x] U-007: tempo source. Resolved: header 0x0C (ticks per second); 0x0A only matters for meta 51h.
- [x] U-034: velocity. Resolved: ignored; level = f(carrier TL, channel volume).
- [x] Loop behaviour. Resolved: §4.8.
- [x] Transpose units. Resolved: §4.7 (22 units per semitone, applied at the next output).
- [ ] U-401 (VE-3, C-0073): §4.4 "the first event runs at tick d0 + 1" disagrees with the capture. `menu.cmf` looping twice: song start → first restart and restart → restart are both 4,841.94 PIT periods (4,841 ticks + 9.8 ms to the restart's PIT write), and the first note sounds 8.8 ms after the song-start PIT write. So the first event runs in the start call, as after a loop restart. The loop restart itself (§4.8) matches write for write ([M3-audio](../../parity/M3-audio.md) rows 12, 12a).

## 8. Provenance

All addresses are `file0001:` image-relative. `file0000` (intro) and `td1` link the same driver at other offsets; `file0000`'s copy is shifted by +1Fh bytes and otherwise identical except for 4 bytes in unrelated code.

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Song start, header fields, divisor 1234DCh div [0Ch] | `2C37:4791..4877` | static + emulation | CONFIRMED |
| Tick loop and event decoding | `2C37:487E..49E4`, `2C37:4A01` (VLQ) | static + emulation + C-0001 | CONFIRMED |
| Dispatch, controllers | `2C37:3E55..3F0B`, `2C37:3D49..3E54`, program change `2C37:423C` | same | CONFIRMED |
| Notes, allocation, output, level | `2C37:3F63..41B8` | same | CONFIRMED |
| Instrument write | `2C37:3BD9..3C9D` | same | CONFIRMED |
| Init and stop | `2C37:41E5..4232`, `2C37:427F..42DB`, `2C37:3CDA` | same | CONFIRMED |
| Tables | `2C37:2F56`, `2F80`, `2F6E`, `2F77`, `2FBF`, `31AB`, `31AC`, `2EC3`, `2E9B` | file bytes | CONFIRMED |
| Whole-stream agreement | C-0001; 17 songs × (one pass + loop) | emulation vs DRO; model vs emulation | CONFIRMED |

## 9. Revision history

| Rev | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-11-02 | main session (SP) | Initial |
| 2 | 2026-11-02 | RE-4 (Archaeologist + Spec author) | WORX playback semantics: clock from 0x0C, −12 note offset, dynamic voice allocation, velocity ignored, volume scaling, transpose step, toggled drums with a fixed kit, lazy instrument loads, loop/stop. Synthetic and capture vectors. Replaces the SBFMDRV assumptions of rev 1 |
