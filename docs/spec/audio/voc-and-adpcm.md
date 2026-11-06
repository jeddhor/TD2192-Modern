# Creative Voice File and Creative ADPCM

| | |
|---|---|
| **Spec ID** | `audio/voc-and-adpcm` |
| **Revision** | 3 |
| **Status** | Draft |
| **Fidelity tier** | T0 for container parsing and ADPCM decoding (the decoded 8-bit stream); T2 for playback (rate, resampling) and for the "playing" status of §2.3 |
| **Confidence** | CONFIRMED. The decoding happens in the Sound Blaster DSP, not in WORX (static: WORX copies the sample bytes unchanged into its DMA buffer). §3 is the DSP algorithm as the oracle (DOSBox-X, SB16) implements it, restated from its behaviour; a decoder written from §3 matches the oracle's WAV output for all three codecs (correlation 0.984..0.999, §4.3). The rate clamp (§2.2) and the playing-status rule (§2.3) are CONFIRMED by static reading plus captures |
| **Tasks** | M3-01, M3-02 |
| **Unknowns** | U-021 (resolved, see [sfx-policy](sfx-policy.md)), U-035, U-400, U-402, U-410 (resolved); U-460 (open, real hardware) |

## 1. Summary

All 17 sound effects are Creative Voice Files. Each holds one sound-data block: either 8-bit unsigned PCM, or one of the three Creative ADPCM schemes (4-bit, "2.6-bit" i.e. 3-bit, and 2-bit). WORX streams the raw bytes to the Sound Blaster by DMA and the DSP decodes the ADPCM in hardware, so the reference decoder is the DSP's. The port decodes in software to 8-bit PCM at the rate the DSP uses (§2.2), and the mixer resamples to the device rate. WORX's "digital sound playing" status stays set for a while after the last sample; §2.3 gives the exact rule, which the effect arbiter and the intro depend on.

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
| 1 | `tc: u8`, `codec: u8`, sample data | New sound data. Rate: §2.2 |
| 2 | Sample data | Continuation of the previous type 1 (same rate and codec; the ADPCM state continues) |
| 3 | `len_minus_1: u16`, `tc: u8` | Silence of `len_minus_1 + 1` samples |
| 4 | `u16` marker | Ignored |
| 5 | ASCII text | Ignored |
| 6 | `u16` repeat count − 1 (`0xFFFF` = forever) | Unsupported. The files don't use it, and parsing reports it. |
| 7 | End of repeat | As above |
| 8 | `u16` tc, `u8` pack, `u8` mode | Unsupported (not used) |
| 9 | New-format data | Unsupported (not used) |

Codec byte: 0 = 8-bit unsigned PCM; 1 = 4-bit ADPCM; 2 = 2.6-bit ADPCM; 3 = 2-bit ADPCM.

Every shipped file is exactly one type-1 block followed by a terminator (all 17 parsed). "Data bytes" D below means the block length − 2: every byte after `tc` and `codec`, the ADPCM reference byte included.

### 2.1 How WORX plays a VOC

The game loads each VOC into memory and plays it with WORX function 06h ([worx.md](../../re/worx.md)). In order:

1. Stop the digital sound that is playing (DSP `D0h` if a transfer is running, DMA channel 1 masked, playing byte := 0), so only one digital sound plays at a time ([sfx-policy](sfx-policy.md)).
2. Playing byte := 1. Fill the whole 4,096-byte DMA ring buffer with `80h`.
3. Skip the 26-byte header and read the first block. For type 1: read `tc` and `codec`; copy the first min(4096, D) data bytes **unchanged** to the start of the ring.
4. Start the DSP: command `40h` with `tc`; program DMA channel 1 for **auto-initialised** single-mode reads over the whole ring (count 0FFFh); then the codec's play command from the table `14h 75h 77h 17h` (PCM, and the "with reference" commands of 4-bit, 2.6-bit, 2-bit ADPCM; U-035), with length bytes `E0h E0h`: one DSP transfer of 57,569 bytes, longer than any shipped sample, so the reference byte is read once per effect.
5. **Refill** (timer streaming, WORX function 3Eh): the second-level work of WORX's timer interrupt (every fast tick that does not chain to the BIOS and finds the driver idle; [worx §4](../../re/worx.md#4-timer-interrupt)) reads DMA channel 1's current address, keeping only its high byte, and refills the part of the ring the DMA has passed since the previous refill: first with silence (`00h` for ADPCM, `80h` for PCM), then with the next data bytes. The exact cadence is in §2.3.
6. When a refill finds that the data is used up (§2.3): DSP `D0h`, DMA masked, read the next block. The terminator ends playback and clears the playing byte.

WORX does not decode, scale or filter the samples: the ADPCM bytes reach the DSP as stored. Loudness and panning are set through the SB Pro mixer ([sfx-policy §4.3](sfx-policy.md#43-volume-and-panning-sound-blaster)). After the data and before the halt the DSP plays fill bytes: `80h` in ADPCM decodes to deltas of 0 (a code with magnitude 0 at step 0), so the last value is held; nothing audible.

Type-2 blocks are appended to the same transfer and type-3 silence is played as a buffer of silence (static reading; no shipped file has them).

### 2.2 Playback rate (U-402)

The DSP limits the time constant per command. In the oracle (DOSBox-X SB16 with its default "sample rate limits", which follow Creative's documented maximum rates):

```text
tc'  = min(tc, LIMIT[codec])
rate = 1,000,000 div (256 − tc')          (integer division; Hz)
```

| Codec | DSP command | LIMIT | Maximum rate |
|---|---|---:|---:|
| 0 PCM (`14h`) | 8-bit single cycle | 234 | 45,454 Hz (SB16; an SB Pro would be 212 / 22,727 Hz) |
| 1 4-bit (`75h`) | ADPCM 4 | 172 | 11,904 Hz |
| 2 2.6-bit (`77h`) | ADPCM 2.6 | 179 | 12,987 Hz |
| 3 2-bit (`17h`) | ADPCM 2 | 165 | 10,989 Hz |

Every shipped VOC by this rule:

| File | tc | Codec | Rate | D | Decoded samples | Length |
|---|---:|---|---:|---:|---:|---:|
| `afterb` | 90 | 4-bit | 6,024 | 4,159 | 8,316 | 1.380 s |
| `boomboom` | 96 | 4-bit | 6,250 | 5,497 | 10,992 | 1.759 s |
| `home` | 131 | 4-bit | 8,000 | 16,116 | 32,230 | 4.029 s |
| `hornet` | 6 | PCM | 4,000 | 4,409 | 4,409 | 1.102 s |
| `income` | 131 | 4-bit | 8,000 | 5,573 | 11,144 | 1.393 s |
| `missile` | 100 | 4-bit | 6,410 | 3,764 | 7,526 | 1.174 s |
| `poink` | 6 | PCM | 4,000 | 4,409 | 4,409 | 1.102 s |
| `pulse` | 6 | PCM | 4,000 | 897 | 897 | 0.224 s |
| `punyexp` | 113 | 2.6-bit | 6,993 | 1,142 | 3,423 | 0.489 s |
| `ric2` | 131 | PCM | 8,000 | 23,941 | 23,941 | 2.993 s |
| `ric3` | 173 → **172** | 4-bit | **11,904** (header 12,048) | 16,764 | 33,526 | 2.816 s |
| `scratch` | 131 | 4-bit | 8,000 | 595 | 1,188 | 0.149 s |
| `stiletto` | 131 | 2-bit | 8,000 | 721 | 2,880 | 0.360 s |
| `vii` | 131 | 4-bit | 8,000 | 1,460 | 2,918 | 0.365 s |
| `vulturei` | 90 | 2-bit | 6,024 | 331 | 1,320 | 0.219 s |
| `whoosh` | 6 | PCM | 4,000 | 17,689 | 17,689 | 4.422 s |
| `zeus` | 131 | 2.6-bit | 8,000 | 1,827 | 5,478 | 0.685 s |

Only `ric3` (an intro effect) is above a limit. The oracle plays it at 11,904 Hz: correlation 0.984 over its whole length at 11,904 Hz against 0.11 at 12,048 Hz (§4.3). What a real Sound Blaster does above the documented rate is not known (U-460); the port follows the oracle.

### 2.3 The "playing" status (WORX function 08h; U-410)

WORX's playing byte (file0001 `CS:31F2`, file0000 `CS:3211`) is cleared only by the refill that finds the data used up, and the refills are paced by the DMA position at 256-byte granularity. So the status outlasts the sample by 0.03..0.75 s depending on the sample. The rule:

**Definitions.**

- R = the DMA byte rate = rate × (bits per code) / 8: rate (PCM), rate / 2 (4-bit), rate / 3 (2.6-bit), rate / 4 (2-bit).
- The ring buffer lies at physical address A = 16 × S + B, where S is the WORX code segment at run time and B the buffer offset in it (`416h` in file0001, td2 and td3; `435h` in file0000), plus 1000h when the 4 KB buffer would cross a 64 KB page (this does not change A mod 256).
- r = (256 − (A mod 256)) mod 256. WORX compares only the high byte of the DMA address, so a refill can happen only when the position in the ring is 400h + r + 256 j for j = 0..11 (positions below 400h are ignored; the wrap from FFFh to 0 is handled at the next of these points).

**Refill points.** Number the positions in bytes since the start of the effect (p = 4096 c + position in the ring, c = 0, 1, ...). A refill happens when the DMA reaches each point below, and each refill *accounts* the ring up to a value:

| Event | Happens when p reaches | Accounts up to |
|---|---|---|
| Normal refill (c ≥ 0, j = 0..11) | 4096 c + 400h + r + 256 j | the same value |
| Wrap refill (c ≥ 1), just before the j = 0 refill of cycle c, at the same position | 4096 c + 400h + r | 4096 c |

**Rule.** Take the events in order. The first event that accounts up to ≥ D finds the data used up (its silence fill covers the end); the **next** event halts the DSP and clears the status. So the status is set for X / R seconds after the effect starts, where X is the "happens when" position of that next event, plus up to one WORX second-level period (at most 2 fast ticks: 20.8 ms with music, 4 ms without).

In words: the status clears at the second refill point at or past the end of the data, where the refill points are every 256 bytes from 400h + r to F00h + r within each 4 KB turn of the ring, and the wrap counts as a point at 4096 c accounted only to the turn boundary.

**r in the oracle** (DOSBox-X loads the first program at PSP 0814h; `td1.exe` starts the intro and Episode 1 at PSP 1398h; nothing starts `td2.exe`/`td3.exe`, they are run directly):

| Program, how started | WORX segment S | A mod 256 | r |
|---|---|---:|---:|
| `file0000` (intro), from `td1` | 16A2h | 55h | **171** |
| `file0001` (E1), from `td1` (the normal path) | 3FDFh | 06h | **250** |
| `file0001` started directly (C-0072, C-0073, C-0074) | 345Bh | C6h | 58 |
| `td2` (E2), run directly | 2EEFh | 06h | **250** |
| `td3` (E3), run directly | 2AFEh | F6h | **10** |

r depends on where DOS loads the program, so on a real machine it is effectively arbitrary (0..255 in steps of 16, plus the offset). Classic uses the oracle's normal-path values in bold.

**Busy time X / R by effect** (seconds; add up to one second-level period):

| Effect | Sample | r = 250 (E1, E2) | r = 10 (E3) | r = 171 (intro) | r = 58 (E1 direct) |
|---|---:|---:|---:|---:|---:|
| `afterb` | 1.380 | 1.868 | 1.788 | 1.842 | 1.804 |
| `boomboom` | 1.759 | 1.882 | 1.887 | 1.857 | 1.903 |
| `home` | 4.029 | 4.095 | 4.354 | **4.395** | 4.367 |
| `hornet` | 1.102 | 1.407 | 1.347 | 1.387 | 1.359 |
| `income` | 1.393 | 1.470 | 1.474 | 1.515 | 1.486 |
| `missile` | 1.174 | 1.276 | 1.601 | 1.651 | 1.616 |
| `pulse` | 0.224 | 0.383 | 0.323 | **0.363** | 0.335 |
| `punyexp` | 0.489 | 0.656 | 0.663 | 0.622 | 0.684 |
| `ric3` | 2.816 | 3.010 | 2.969 | **2.996** | 2.977 |
| `scratch` | 0.149 | 0.383 | 0.323 | **0.363** | 0.335 |
| `stiletto` | 0.360 | 0.765 | 0.645 | 0.726 | 0.669 |
| `vii` | 0.365 | 0.447 | 0.451 | 0.491 | 0.463 |
| `vulturei` | 0.219 | 1.016 | 0.857 | 0.963 | 0.888 |
| `whoosh` | 4.422 | 4.543 | 4.546 | **4.523** | 4.495 |
| `zeus` | 0.685 | 0.862 | 0.868 | 0.832 | 0.790 |

Bold: the intro effects measured in C-0081 (per-frame RAM of the playing byte, [intro](../flow/intro.md) §3.6): `whoosh` 4.523 s, `pulse` and `scratch` 25–26 frames (0.357..0.371 s), `ric3` 211 frames (3.011 s), `home` 309 frames (4.409 s). Every prediction is within one frame plus one second-level period of the measurement. The times do not depend on the WORX timer rate (C-0071, music off, gives the same), only on the DMA position. In E1 started directly (r = 58), the arbiter's priority reset after a `boomboom` (priority 5) comes 1.926 s after the frame that first shows the priority (C-0072 frames 1441 → 1576, ticks at 18.52 Hz): predicted 1.903 s plus less than one tick.

Port (Classic): model the playing status per effect as "set from the start until X / R seconds have elapsed" (X from the rule, r per program as in bold), and let the sfx arbiter ([sfx-policy §4.2](sfx-policy.md#42-priority-and-preemption-device-2)), the end-of-mission wait and the intro's effect rules ([intro](../flow/intro.md) §3.6) read that status, not the end of the audio. A new effect resets the status at its own start (step 1 of §2.1).

## 3. ADPCM decoding (the DSP's algorithm)

State: `value`, an unsigned 8-bit sample (0..255; PCM = `value`, centred = `value − 128`), and `step`, an integer 0..TOP.

1. **Reference byte.** The first data byte of a type-1 block sets `value` and `step := 0`. It is **not** output as a sample (the oracle's DSP only uses it as the starting value). WORX starts every type-1 block with the "with reference" command (U-035). A type-2 block continues the transfer: no reference byte, the state carries on.
2. **Code unpacking**, most significant bits first:

   | Codec | Codes per byte | Codes of byte b |
   |---|---:|---|
   | 4-bit | 2 | `b >> 4`, `b & 0Fh` |
   | 2.6-bit | 3 | `b >> 5`, `(b >> 2) & 7`, `(b & 3) << 1` |
   | 2-bit | 4 | `b >> 6`, `(b >> 4) & 3`, `(b >> 2) & 3`, `b & 3` |

   A code of width w (4, 3 or 2 bits) has a sign bit (its top bit) and a magnitude m (the other w − 1 bits). The third 2.6-bit code is the last two bits shifted up: its sign is bit 1 of the byte and its magnitude is 0 or 2, so it can step down but never step up.
3. **Per code:**

   ```text
   delta = m                          if step = 0
         = (2·m + 1) × 2^(step − 1)   if step ≥ 1        (except 2.6-bit at step 4: (2·m + 1) × 5)
   value = clamp(value − delta if sign else value + delta, 0, 255)
   output value
   if m = 0:                          step = max(step − 1, 0)
   else if m ≥ UP and step < TOP:     step = step + 1
   ```

   | Codec | m range | UP | TOP |
   |---|---|---:|---:|
   | 4-bit | 0..7 | 5 | 3 |
   | 2.6-bit | 0..3 | 3 | 4 |
   | 2-bit | 0..1 | 1 | 5 |

   Note that a magnitude-0 code still moves the value by 2^(step − 1) when step ≥ 1 (a "half step" in the sign's direction), and that the step changes after the delta is applied.

The delta tables this gives (positive codes; a set sign bit negates):

| Codec | Step | Deltas for m = 0, 1, 2, ... |
|---|---:|---|
| 4-bit | 0 | 0 1 2 3 4 5 6 7 |
| | 1 | 1 3 5 7 9 11 13 15 |
| | 2 | 2 6 10 14 18 22 26 30 |
| | 3 | 4 12 20 28 36 44 52 60 |
| 2.6-bit | 0 | 0 1 2 3 |
| | 1 | 1 3 5 7 |
| | 2 | 2 6 10 14 |
| | 3 | 4 12 20 28 |
| | 4 | **5 15 25 35** |
| 2-bit | 0 | 0 1 |
| | 1 | 1 3 |
| | 2 | 2 6 |
| | 3 | 4 12 |
| | 4 | 8 24 |
| | 5 | 16 48 |

Decoded sample counts for n data bytes: 4-bit 2 (n − 1), 2.6-bit 3 (n − 1), 2-bit 4 (n − 1).

**What changed from rev 2.** Rev 2 used delta = m × 2^(step + shift) with a shift per codec, a top step of 3 for every codec, and output the reference byte. That correlated 0.26..0.86 with the oracle (BUG-4). VE-3's black-box fit (M3-audio row 14) found the rule above except for the 2.6-bit step 4, where it assumed × 8: the two differ in only 72 of `punyexp`'s 3,423 samples, and × 5 fits the WAV better (residual 94.8 against 96.2, §4.3).

## 4. Test vectors

### 4.1 Synthetic (from §3)

Data bytes in hex (the first is the reference); output samples in hex.

| # | Codec | Data | Output |
|---|---|---|---|
| 1 | 4-bit | `80 70` | `87 88` (code 7: +7, step 1; code 0: +1, step 0) |
| 2 | 4-bit | `80 F8` | `79 78` (code F: −7, step 1; code 8: −1, step 0) |
| 3 | 4-bit | `80 77 77` | `87 96 B4 F0` (deltas 7, 15, 30, 60; step stays 3) |
| 4 | 4-bit | `FE 77` | `FF FF` (clamp) |
| 5 | 4-bit | `80 77 70 00` | `87 96 B4 B8 BA BB` (the zero codes at steps 3, 2, 1 add 4, 2, 1) |
| 6 | 4-bit | `03 FF` | `00 00` (clamp at 0) |
| 7 | 2-bit | `80 55 55` | `81 84 8A 96 AE DE FF FF` (steps 0..5, deltas 1 3 6 12 24 48) |
| 8 | 2-bit | `80 55 00` | `81 84 8A 96 9E A2 A4 A5` (zero codes at steps 4, 3, 2, 1 add 8, 4, 2, 1) |
| 9 | 2-bit | `80 FF` | `7F 7C 76 6A` |
| 10 | 2.6-bit | `80 6D 6D 60` | `83 8A 94 A2 BE D7 FA FF FF` (step 4 deltas 25 and 35, then the zero code at step 4 adds 5) |
| 11 | 2.6-bit | `80 FF` | `7D 76 6C` (third code: sign set, m = 2) |
| 12 | 2.6-bit | `80 03` | `80 80 7E` (third code `110`: sign set, m = 2, at step 0: −2) |

### 4.2 Real files

SHA-256 of the decoded 8-bit output (all samples, §3), first 16 hex digits; sample counts as in §2.2.

| File | SHA-256 (prefix) | File | SHA-256 (prefix) |
|---|---|---|---|
| `afterb` | `0ea3db570d6efe55` | `punyexp` | `634b47fd5df7ce0e` |
| `boomboom` | `78a7a1289d7dfeca` | `ric2` | `8d63c7d6bad0d34b` |
| `home` | `7c39168b2243ac75` | `ric3` | `2916617f4cccdceb` |
| `hornet` | `c1ef92309ab534fd` | `scratch` | `b9f30973a47c2f81` |
| `income` | `938defcb857d54ad` | `stiletto` | `7fe078fe09967221` |
| `missile` | `b3e38d9dd542843a` | `vii` | `c13a7450190bbc9c` |
| `poink` | `6b1eda6d402c8894` | `vulturei` | `f2ce2a7b7c6a7b6c` |
| `pulse` | `a1ded05990d84da2` | `whoosh` | `dfe0401a58dd882f` |
| | | `zeus` | `2ff1ac4b975e2488` |

PCM hashes are of the data bytes themselves.

### 4.3 Against the oracle

Decoded with §3, resampled linearly from the §2.2 rate to 48 kHz, correlated (mean removed) with the left channel of the oracle WAV at the onset found by search (RE-16 scratch tools; unpositioned effects, L = R):

| Effect | Capture | Onset | Length compared | Correlation | Rev 2 rule |
|---|---|---:|---:|---:|---:|
| `boomboom` (4-bit) | C-0072 | 20.568 s | 1.7 s | 0.9975 | 0.834 |
| `home` (4-bit) | C-0071 | 36.790 s | 4.0 s | 0.9878 | 0.454 |
| `ric3` (4-bit, 11,904 Hz) | C-0071 | 31.939 s | 2.8 s | 0.9838 (0.115 at 12,048 Hz) | 0.324 |
| `punyexp` (2.6-bit) | C-0072 | 32.276 s | 0.49 s | 0.9969 (× 8 at step 4: 0.9961) | 0.872 |
| `vulturei` (2-bit) | C-0072 | 24.065 s | 0.22 s | 0.9992 | 0.310 |

A port test can repeat this: decode, resample, and require ≥ 0.98 at the best lag within ±10 ms of these onsets (needs `TD2192_ORACLE`). The remaining difference is the oracle's output filter and resampler.

## 5. Open questions

- [x] U-035: resolved. WORX picks the DSP command from the table at `file0001:2C37:31DC` (`14 75 77 17 14 74 76 16`), indexed by the block's codec byte. Codecs 0..3 select the "with reference" commands. The "no reference" entries (index 4..7) are reachable only through WORX function 3Fh, which the game does not call.
- [x] U-400: resolved (RE-16): the decoding is the DSP's; §3 restated; vectors §4.
- [x] U-402: resolved (RE-16): the oracle's DSP clamps the 4-bit time constant at 172 (§2.2).
- [x] U-410: resolved (RE-16): §2.3.
- [ ] U-460: does a real Sound Blaster decode exactly as §3 (reference byte not output, 2.6-bit step 4 × 5) and clamp the rates of §2.2? Unknown; the oracle defines Classic.
- [x] Does WORX apply any volume scaling or filtering to SFX? No: samples go to the DSP unchanged. The game sets the SB Pro mixer voice volume per effect (left/right panning and distance), see [sfx-policy](sfx-policy.md).

## 6. Provenance

| Claim | Source | Confidence |
|---|---|---|
| Container layout and block types | Public VOC format documentation; parse of all 17 files (one type-1 block + terminator each) | CONFIRMED |
| WORX copies the data unchanged; ring buffer, DSP start, length E0E0h | `file0001:2C37:62D5` (fn 06h), `631F` (fill/next block), `643F` (type 1), `6514` (copy), `4CF9` (DSP start: `40h`, tc, DMA mode 59h, count 0FFFh, command via `31DC`, length `E0h E0h`) | CONFIRMED (static) |
| Buffer address 16 × S + 416h (+1000h across a page); file0000 435h | `2C37:4B95..4BCA` (fn 01h); the immediate in each exe (file0001, td2, td3 416h; file0000 435h); segments from C-0002/C-0072/C-0120/C-0125 `int21.tsv` and `pit.tsv` (PSP 1398h / 0814h) | CONFIRMED |
| Refill only at DMA high-byte positions 400h..1000h, wrap handling, data-used-up test, halt and status clear | `2C37:61FA` (refill entry), `4DC4..4E19` (DMA position), `64F0` (data accounting), `631F..63C6` (halt, terminator → `4B49` stop) | CONFIRMED (static) + C-0081, C-0071, C-0072 |
| ADPCM algorithm and tables | Behaviour of the oracle's DSP emulation (DOSBox-X `sblaster`, read as a tool, restated here; no code copied) | CONFIRMED for the oracle (static + WAV correlation, §4.3) |
| Rate clamp per codec | Same source ("sample rate limits", default on); `ric3` rate scan C-0071 | CONFIRMED for the oracle |
| Reference byte at the start of each type-1 block | DSP command table `file0001:2C37:31DC`; type-1 handler `2C37:643F` | CONFIRMED (static) |

## 7. Revision history

| Rev | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-11-02 | main session (SP) | Initial |
| 2 | 2026-11-02 | RE-4 (Archaeologist) | U-035 confirmed from the DSP commands WORX uses; how WORX plays VOCs (§2.1) |
| 3 | 2026-11-06 | RE-16 (Archaeologist + Spec author) | BUG-4 / U-400: §3 restated as the DSP's algorithm (deltas (2m + 1)·2^(step − 1), 2.6-bit step 4 × 5, tops 3/4/5, reference byte not output); synthetic, file-hash and oracle vectors (§4). U-402: per-codec time-constant clamp (§2.2, `ric3` 11,904 Hz). U-410: exact playing-status rule (§2.3). §2.1 corrected (ring fill, DSP length, refill) |
