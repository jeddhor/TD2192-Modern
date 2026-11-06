# Creative Voice File and Creative ADPCM

| | |
|---|---|
| **Spec ID** | `audio/voc-and-adpcm` |
| **Revision** | 2 |
| **Status** | Draft |
| **Fidelity tier** | T0 for container parsing and ADPCM decoding; T2 for playback (resampling) |
| **Confidence** | LIKELY: written from the public descriptions of the VOC container and the Sound Blaster ADPCM schemes. The reference-byte rule is CONFIRMED from the DSP commands WORX issues (§3, U-035). Decoder output still has to be checked against a DOSBox-X capture (M3-08). |
| **Tasks** | M3-01, M3-02 |
| **Unknowns** | U-021 (resolved, see [sfx-policy](sfx-policy.md)), U-035 (resolved) |

## 1. Summary

All 17 sound effects are Creative Voice Files. Each holds one sound-data block: either 8-bit unsigned PCM, or one of the three Creative ADPCM schemes that the Sound Blaster DSP decoded in hardware (4-bit, "2.6-bit" i.e. 3-bit, and 2-bit). The port decodes them in software to 8-bit PCM at the file's own rate. The mixer then resamples to the device rate.

## 2. Container

| Offset | Size | Field |
|---:|---:|---|
| 0 | 20 | `"Creative Voice File"` + `0x1A` |
| 20 | 2 | Offset of the first block (26 in our files) |
| 22 | 2 | Version |
| 24 | 2 | Check word (`!version + 0x1234`); not validated |

Blocks follow: a type byte, then (except for type 0) a 24-bit little-endian length and that many bytes.

| Type | Content | Port behaviour |
|---|---|---|
| 0 | Terminator (no length) | Stop |
| 1 | `tc: u8`, `codec: u8`, sample data | New sound data. Rate = `1_000_000 / (256 − tc)` Hz |
| 2 | Sample data | Continuation of the previous type 1 (same rate and codec; the ADPCM state continues) |
| 3 | `len_minus_1: u16`, `tc: u8` | Silence of `len_minus_1 + 1` samples |
| 4 | `u16` marker | Ignored |
| 5 | ASCII text | Ignored |
| 6 | `u16` repeat count − 1 (`0xFFFF` = forever) | Unsupported. The files don't use it, and parsing reports it. |
| 7 | End of repeat | As above |
| 8 | `u16` tc, `u8` pack, `u8` mode | Unsupported (not used) |
| 9 | New-format data | Unsupported (not used) |

Codec byte: 0 = 8-bit unsigned PCM; 1 = 4-bit ADPCM; 2 = 2.6-bit ADPCM; 3 = 2-bit ADPCM.

### 2.1 How WORX plays a VOC (for reference)

The game loads each VOC into memory and plays it with WORX function 06h ([worx.md](../../re/worx.md)). WORX first stops the sound that is playing, so only one digital sound plays at a time ([sfx-policy](sfx-policy.md)). It then skips the 26-byte header and parses blocks. For a type-1 block it sends DSP `40h` with the block's time constant, programs DMA channel 1 in auto-init mode over a 4 KB buffer, and issues one long DSP transfer with the codec's command (§3). As the DSP consumes the buffer, WORX refills it from the timer interrupt; this is the "timer streaming" option the game turns on with function 3Eh. Type-2 blocks are appended to the same transfer. Type-3 silence is played as a buffer of silence. When the data ends, the rest of the buffer is filled with silence (80h for PCM, 00h for ADPCM) until WORX halts the DSP (`D0h`). Blocks of type 0 and 4–7 end playback. WORX does not scale or filter the samples: SFX loudness and panning are set through the SB Pro mixer ([sfx-policy §4.3](sfx-policy.md#43-volume-and-panning-sound-blaster)).

## 3. ADPCM decoding

State: `sample` (signed, range −128..=127, centred so that unsigned PCM = `sample + 128`) and `step` (0..=3).

1. **Reference byte:** the first data byte of each type-1 block is an uncompressed unsigned 8-bit sample. It becomes the first output sample and initialises `sample = byte − 128`, with `step = 0`. CONFIRMED (U-035): WORX starts every type-1 block with the DSP's "with reference" command for the block's codec (`75h` 4-bit, `77h` 2.6-bit, `17h` 2-bit; `14h` for 8-bit PCM). A type-2 block continues the same DSP transfer, so it has no reference byte and the ADPCM state carries on (§2).
2. **Code unpacking**, most significant bits first:
   - 4-bit: two codes per byte, `b >> 4` then `b & 0x0F`. Code width 4.
   - 2.6-bit: three codes per byte, `b >> 5`, `(b >> 2) & 7`, `(b & 3) << 1`. Code width 3; the last code has only 2 significant bits.
   - 2-bit: four codes per byte, `b >> 6`, `(b >> 4) & 3`, `(b >> 2) & 3`, `b & 3`. Code width 2.
3. **Per code** of width `w` (with `shift = 4 − w`: 0, 1 and 2 for 4, 3 and 2 bits):
   - `sign` = top bit of the code; `mag` = the remaining `w − 1` bits.
   - `delta = mag << (step + shift)`
   - `sample = clamp(sample ± delta, −128, 127)` (minus when `sign` is set).
   - Step adaptation: if `mag` is the largest-but-maybe-one value (`mag ≥ 2w − 3`, i.e. ≥ 5, ≥ 3, ≥ 1 for widths 4, 3, 2) and `step < 3`, then `step += 1`; else if `mag == 0` and `step > 0`, then `step -= 1`.
   - Output `sample + 128`.

## 4. Test vectors

| # | Input | Expected |
|---|---|---|
| 1 | 4-bit, data `80 70` | `80` (reference), then code 7 → +7 (step → 1) = `87`, code 0 → +0 (step → 0) = `87` |
| 2 | 4-bit, data `80 F8` | `80`, code F (−7) → `79` (step 1), code 8 (−0, mag 0) → `79` (step 0) |
| 3 | 2-bit, data `80 55` | `80`, then four codes `1` (+mag 1, shift 2): `84` (step 1), `8C` (step 2), `9C` (step 3), `BC` (step 3) |
| 4 | Clamp: 4-bit, data `FE 77 77` | Saturates at `FF` |
| 5 | Real files | Decoded sample counts: 4-bit = 1 + 2·(n−1), 2.6-bit = 1 + 3·(n−1), 2-bit = 1 + 4·(n−1), where n = data bytes |

Vectors 1–4 follow from §3 by hand (they test the description, not the original). The real check is the A/B comparison with DOSBox-X captures (M3-08).

## 5. Open questions

- [x] U-035: resolved. WORX picks the DSP command from the table at `file0001:2C37:31DC` (`14 75 77 17 14 74 76 16`), indexed by the block's codec byte. Codecs 0..3 select the "with reference" commands. The "no reference" entries (index 4..7) are reachable only through WORX function 3Fh, which the game does not call.
- [ ] U-400 (VE-3, M3-08 A/B): §3 does not match the oracle. Correlation of the §3 decode with DOSBox-X's output: 4-bit `scratch` 0.86, `home` 0.26, 2.6-bit `punyexp` 0.85, 2-bit `vulturei` 0.78. A rule that fits (0.988..0.9997, at the PCM gain): delta = m at step 0 and (2m + 1) × 2^(step − 1) at steps ≥ 1, no shift, top step 3 (4-bit), 4 (2.6-bit), 5 (2-bit), same step up/down conditions. Black-box fit; see [M3-audio](../../parity/M3-audio.md) row 14 and BUG-4.
- [ ] U-402 (VE-3): `ric3.voc` (tc 173) plays at 1,000,000/84 = 11,905 Hz in the oracle, not 12,048 Hz.
- [x] Does WORX apply any volume scaling or filtering to SFX? No: samples go to the DSP unchanged. The game sets the SB Pro mixer voice volume per effect (left/right panning and distance), see [sfx-policy](sfx-policy.md).

## 6. Provenance

| Claim | Source | Confidence |
|---|---|---|
| Container layout and block types | Public VOC format documentation; header parse of all 17 files (FORMATS.md §15) | CONFIRMED (container) |
| ADPCM algorithm | Public descriptions of the SB DSP ADPCM schemes, restated here | LIKELY |
| Reference byte at the start of each type-1 block | DSP command table `file0001:2C37:31DC`; type-1 handler `2C37:643F` (codec byte → `2C37:4CEC`) and DSP start `2C37:4CF9` (command via the table, after `40h` + time constant) | CONFIRMED (static) |

## 7. Revision history

| Rev | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-11-02 | main session (SP) | Initial |
| 2 | 2026-11-02 | RE-4 (Archaeologist) | U-035 confirmed from the DSP commands WORX uses; how WORX plays VOCs (§2.1) |
