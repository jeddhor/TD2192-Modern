# Creative Voice File and Creative ADPCM

| | |
|---|---|
| **Spec ID** | `audio/voc-and-adpcm` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T0 for container parsing and ADPCM decoding; T2 for playback (resampling) |
| **Confidence** | LIKELY: written from the public descriptions of the VOC container and the Sound Blaster ADPCM schemes. Decoder output still has to be checked against a DOSBox-X capture (M3-08). |
| **Tasks** | M3-01, M3-02 |
| **Unknowns** | U-021 (SFX policy), U-035 (reference byte) |

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

## 3. ADPCM decoding

State: `sample` (signed, range −128..=127, centred so that unsigned PCM = `sample + 128`) and `step` (0..=3).

1. **Reference byte:** the first data byte of each type-1 block is an uncompressed unsigned 8-bit sample. It becomes the first output sample and initialises `sample = byte − 128`, with `step = 0`. (Assumption U-035: all our ADPCM files start with a reference byte, as the DSP's "with reference" commands expect.)
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

- [ ] U-035: is the first byte always a reference byte in these files (as assumed), or does WORX use the "no reference" DSP commands for some?
- [ ] Does WORX apply any volume scaling or filtering to SFX?

## 6. Provenance

| Claim | Source | Confidence |
|---|---|---|
| Container layout and block types | Public VOC format documentation; header parse of all 17 files (FORMATS.md §15) | CONFIRMED (container) |
| ADPCM algorithm | Public descriptions of the SB DSP ADPCM schemes, restated here | LIKELY |

## 7. Revision history

| Rev | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-11-02 | main session (SP) | Initial |
