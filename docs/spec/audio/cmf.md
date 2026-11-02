# Creative Music File playback

| | |
|---|---|
| **Spec ID** | `audio/cmf` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T2 (perceptual), pending WORX-specific findings |
| **Confidence** | Container CONFIRMED (all 17 files); sequencer behaviour LIKELY (standard CMF semantics plus our data survey); WORX deviations UNKNOWN (U-007, U-029) |
| **Tasks** | M3-04, M3-05 |

## 1. Summary

Music is 17 Creative Music Files (CMF v1.01) for a Yamaha YM3812 (OPL2). A CMF is a header, a bank of 16-byte OPL instrument definitions, and a single MIDI-like event track. The port parses the file (`td-formats::cmf`) and plays it with a sequencer (`td-audio::cmf_player`) that writes OPL2 registers into `td-opl`.

## 2. Data

### 2.1 Header (v1.01)

| Offset | Type | Field |
|---:|---|---|
| 0x00 | 4 bytes | `CTMF` |
| 0x04 | u16 | Version (0x0101) |
| 0x06 | u16 | Instrument block offset |
| 0x08 | u16 | Music data offset |
| 0x0A | u16 | Ticks per quarter note (varies 23..192; informational) |
| 0x0C | u16 | Ticks per second (96 in every file): **the playback clock** (U-007 to confirm for WORX) |
| 0x0E | 3 × u16 | Title, composer, remarks offsets (0 = none) |
| 0x14 | 16 × u8 | Channel-in-use flags |
| 0x24 | u16 | Instrument count |
| 0x26 | u16 | Basic tempo (informational) |

### 2.2 Instrument (16 bytes)

| Byte | OPL register (operator 1 = modulator, operator 2 = carrier) |
|---:|---|
| 0, 1 | 0x20 + op: AM / VIB / EG type / KSR / multiplier (mod, car) |
| 2, 3 | 0x40 + op: key-scale level / output level (mod, car) |
| 4, 5 | 0x60 + op: attack / decay (mod, car) |
| 6, 7 | 0x80 + op: sustain / release (mod, car) |
| 8, 9 | 0xE0 + op: wave select (mod, car) |
| 10 | 0xC0 + voice: feedback / connection |
| 11–15 | Unused |

### 2.3 Events

Standard MIDI channel messages with variable-length delta times (in ticks) and running status: note off `8n`, note on `9n` (velocity 0 = off), polyphonic aftertouch `An`, controller `Bn`, program `Cn`, channel pressure `Dn`, pitch bend `En`. Meta `FF 2F 00` ends the track. SysEx `F0/F7` is skipped by length.

CMF controllers used by our files:

| Controller | Meaning | Use in our files |
|---|---|---|
| 0x63 (99) | AM/VIB depth: bit 1 → reg 0xBD bit 7 (AM depth), bit 0 → bit 6 (VIB depth) | not used |
| 0x66 (102) | Marker (exposed to the game) | not used |
| 0x67 (103) | Rhythm mode: nonzero = on | once at the start of every song (1 in 16 songs, 0 in `moodmaj`) |
| 0x68 (104) | Transpose up, in 1/128 semitone | not used |
| 0x69 (105) | Transpose down, in 1/128 semitone | `satair`, `td01`, `td03`, `moodmaj` (sliding values: a pitch effect) |

## 3. Behaviour

### 3.1 Timing

The sequencer advances at **96 ticks per second** (header field 0x0C). All events due at a tick are processed before audio for that tick is rendered. At the end of the track the song **loops** to its first event, resetting controllers but not the OPL state (U-007: loop behaviour unconfirmed).

### 3.2 Initialisation

Reset the chip; write `0x01 = 0x20` (enable wave select); every voice key-off; `0xBD = 0`.

### 3.3 Channel mapping (fixed)

Observed: every song uses melodic MIDI channels 0..=5 (0..=8 in `moodmaj`) and percussion channels 11..=15. Each channel plays at most one note at a time, except that channel 0 sometimes overlaps two notes.

| MIDI channel | Rhythm mode off | Rhythm mode on |
|---|---|---|
| 0..=5 | OPL voice = channel | OPL voice = channel |
| 6..=8 | OPL voice = channel | (unused) |
| 11 | — | Bass drum (voice 6, both operators) |
| 12 | — | Snare drum (voice 7 carrier, operator offset 0x14) |
| 13 | — | Tom-tom (voice 8 modulator, offset 0x12) |
| 14 | — | Top cymbal (voice 8 carrier, offset 0x15) |
| 15 | — | Hi-hat (voice 7 modulator, offset 0x11) |

A note-on on a voice that is already sounding replaces that note (key-off, then key-on).

### 3.4 Program change

Melodic voice or bass drum: write the full instrument to both operators and `0xC0 + voice`. Single-operator percussion: write the instrument's **modulator** bytes for modulator-slot drums (tom-tom, hi-hat) and its **carrier** bytes for carrier-slot drums (snare, cymbal).

### 3.5 Notes

- Frequency of MIDI note `k` with fine transpose `t` (1/128 semitone): `f = 440 × 2^((k − 69)/12 + t/(12 × 128))`. Choose the smallest block `b` (0..=7) such that `fnum = round(f × 2^(20 − b) / 49716) ≤ 1023`.
- Melodic note-on: write `0xA0 + v = fnum & 0xFF` and `0xB0 + v = 0x20 | b << 2 | fnum >> 8`. Note-off clears bit 0x20.
- Percussion note-on: set the drum's pitch voice frequency (bass drum: voice 6; snare and hi-hat: voice 7; tom-tom and cymbal: voice 8) without key-on, then set the drum's bit in `0xBD` (BD 0x10, SD 0x08, TT 0x04, CY 0x02, HH 0x01), clearing it first if it is already set. Note-off clears the bit.
- **Velocity is ignored** (assumption U-029; WORX behaviour unknown).

### 3.6 Transpose

Controllers 0x68/0x69 set the channel's transpose to `+value`/`−value` (in 1/128 semitone). The new value applies to the channel's next note-on.

## 4. Test vectors

| # | Input | Expected |
|---|---|---|
| 1 | Note 69 (A4), no transpose | f = 440 Hz → block 4, fnum 580 (`0x244`) |
| 2 | Note 60 (C4) | f ≈ 261.63 Hz → block 3, fnum ≈ 690 |
| 3 | Durations at 96 ticks/s | climax 53.8 s, death 28.8 s, hangar 48.1 s, intro 84.0 s, td01 116.0 s, vulture 100.0 s (track length / 96) |

## 5. Open questions

- [ ] U-007: does WORX play at the header's 96 ticks/s?
- [ ] U-029: does WORX apply velocity (carrier attenuation)?
- [ ] Loop behaviour at end of track (loop, stop, or hand back to the game).
- [ ] Exact transpose semantics (units, and whether it applies to sounding notes).

## 6. Provenance

| Claim | Source | Confidence |
|---|---|---|
| Header fields, instrument layout, controller numbers | Public CMF documentation; header parse of all 17 files | CONFIRMED (fields) |
| Channel usage, controllers used, durations | Data survey of all 17 files (2026-11-02) | CONFIRMED |
| Fixed channel-to-voice mapping, percussion slots | Standard CMF (SBFMDRV) behaviour, consistent with the survey | LIKELY |

## 7. Revision history

| Rev | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-11-02 | main session (SP) | Initial |
