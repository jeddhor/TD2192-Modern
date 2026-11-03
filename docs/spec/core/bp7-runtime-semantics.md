# Borland Pascal 7 runtime semantics

| | |
|---|---|
| **Spec ID** | `core/bp7-runtime-semantics` |
| **Revision** | 2 |
| **Status** | Draft |
| **Fidelity tier** | T1 for `Random` and Real48 arithmetic (gameplay state depends on them); T3 for everything the port replaces (file I/O, Crt text, Halt) |
| **Confidence** | RNG: CONFIRMED (static, two independent derivations). Real48 usage and operation details: CONFIRMED (bit-exact algorithms in [real48.md](real48.md)) |
| **Tasks** | M1-01 (follow-ups: M1-01.b Real48 bit-exact, VE RandSeed watch) |
| **Unknowns** | U-022 (resolved), U-023 (resolved), U-029 (resolved), U-030, U-031 |

## 1. Summary

All three episode programs are Borland Pascal 7 executables. They carry the BP7 **System** unit (start-up, heap, file I/O, strings, LongInt helpers, software Real48 arithmetic, `Random`) plus the **Crt** and **Dos** units. The port only reproduces the parts that affect game state:

1. **`Random`.** It is a 32-bit linear congruential generator that is **never seeded from the clock**: the game never calls `Randomize` and never writes `RandSeed`, so every run of an episode program starts from seed 0.
2. **Real48 software floating point.** Game code calls the System Real48 routines from about 470 call sites (add, multiply, divide, compare, LongInt→Real, Round, Sqrt, Sin, Cos, ArcTan). This confirms risk R11: `td-sim` needs a bit-exact Real48 type.
3. **Integer semantics.** The game was compiled with `{$R-,Q-}` (no range or overflow checks), so integer arithmetic wraps silently. LongInt multiply/divide go through System helpers with the usual two's-complement results.

This spec gives the exact `Random` algorithm with test vectors, the Real48 format and the inventory of Real48 entry points that game code uses, and the unit/segment map of `file0001.exe`. The bit-exact Real48 algorithms (add, multiply, divide, compare, conversions, `Sqrt`, `Sin`, `Cos`, `ArcTan`) are in [real48.md](real48.md).

## 2. Definitions and units

| Term / quantity | Type in original | Unit | Range | Notes |
|---|---|---|---|---|
| `RandSeed` | `LongInt` (stored as u32) | — | 0..2³²−1 | DS:04EE (low word), DS:04F0 (high word). Initial value 0 |
| LCG multiplier | constant | — | `0x08088405` | Low word `0x8405` is a code constant; high word `0x0808` is implied by shifts |
| `Random(Range)` | `function(Range: Word): Word` | — | 0..Range−1 (0 when Range = 0) | `Range` is unsigned 16-bit |
| `Random` (no argument) | `function: Real` | — | [0, 1) | Real48 result |
| Real48 | 6 bytes | — | ±2.9e−39..1.7e38 | Borland `Real` (§3.3) |
| Image-relative address | `seg:off` | — | — | ADG §7.8; `file0001:` prefix implied in this spec |

## 3. Data

### 3.1 Unit / code-segment map (`file0001.exe`)

The entry point is the main block. It calls each unit's initialisation part in `uses` order, System first. The linker laid the segments out in reverse order, with System last. Units without an initialisation part sit between the others. Roles are ours, inferred from hardware access and runtime-call patterns (confidence in brackets). Exact segment sizes and the E2/E3 equivalents are in [EXECUTABLES.md §1.1](../../re/EXECUTABLES.md#11-ghidra-image-base-segments-and-dgroup-m0-08-2026-11-02).

| Seg | Size | Init (call order) | Role (our name) | Evidence |
|---|---:|---|---|---|
| `0000` | 51A0 | — (entry `0000:4526`) | Main program | Init chain at entry; most `ParamStr`/`WriteLn` calls; config and switches |
| `051A` | 9710 | `051A:96F9` (8th) | Front end / screens / loaders (HYPOTHESIS) | Most `Assign`/`Reset`/`BlockRead` calls; `KeyPressed`/`ReadKey`; `Delay` |
| `0E8B` | F9F0 | `0E8B:F9E7` (7th) | Game engine: flight, AI, keyboard ISR (LIKELY) | `IN 60h` + EOI; most Real48 and `Random` calls; `0E8B:B880` (16.7 KB) |
| `1E2A` | 0950 | — | Sound-effect manager (LIKELY) | VOC filename table; `Random`; `Sound`/`NoSound`; `SetIntVec` |
| `1EBF` | 01F0 | — | Joystick (CONFIRMED) | Port 201h read loop |
| `1EDE` | 5260 | `1EDE:5247` (6th) | Mission text and objectives (LIKELY) | Fixed-width objective strings; `1EDE:0E6A` does 698 string stores |
| `2404` | 6110 | `2404:60FD` (5th) | Unknown game unit | — |
| `2A15` | 2220 | `2A15:220F` (4th) | Dossiers / personnel text (LIKELY) | Character names and ranks; 186 string stores |
| `2C37` | 65B0 | — | **WORX Toolkit 2.1** (CONFIRMED) | SB/OPL/MPU/DMA/PIT ports, `WAVEfmt ` |
| `3292` | 2540 | — | Unknown (2 functions; Real48 user) | — |
| `34E6` | 3880 | `34E6:3836` (3rd) | Engine support unit (identical size in E1–E3) | `Random`, Real `Random`, `Sqrt` |
| `386E` | 1AE0 | — | VGA graphics (CONFIRMED) | Sequencer/GC/CRTC/DAC ports, mode set, retrace |
| `3A1C` | 0620 | `3A1C:000D` (2nd) | **Crt** (CONFIRMED) | §4.5 |
| `3A7E` | 0140 | — | **Dos** (CONFIRMED) | GetDate/SetDate/GetTime/SetTime/GetIntVec/SetIntVec/Exec |
| `3A92` | 0080 | — | Small library unit (one routine) | — |
| `3A9A` | 1A90 | `3A9A:0000` (1st) | **System** (CONFIRMED) | Start-up code loads DGROUP |
| `3C43` | 0510 + BSS | — | DGROUP (data) | `mov dx,3C43h; mov ds,dx` in System init |

### 3.2 System typed constants in DGROUP

These are in the standard BP7 order, starting at DS:04B2. Values are initial values from the file.

| DS offset | Name | Type | Initial | Notes |
|---|---|---|---|---|
| 04C4 | `OvrEmsHandle` | u16 | FFFFh | |
| 04C6 | `HeapOrg` | far ptr | set at start-up | |
| 04CA | `HeapPtr` | far ptr | set at start-up | |
| 04CE | `HeapEnd` | far ptr | PSP top of memory | |
| 04D2 | `FreeList` | far ptr | | |
| 04DE | `ExitProc` | far ptr | nil | |
| 04E2 | `ExitCode` | i16 | 0 | |
| 04E8 | `PrefixSeg` | u16 | PSP segment | |
| 04EC | `InOutRes` | i16 | 0 | I/O error code |
| **04EE** | **`RandSeed`** | **u32** | **0** | **Only the LCG step reads or writes it** |
| 04F2 | `SelectorInc` | u16 | 1000h | |
| 04F4–04FA | `Seg0040`, `SegA000`, `SegB000`, `SegB800` | u16 | 0040h, A000h, B000h, B800h | |
| 04FC | `Test8086` | u8 | set at start-up | ≥ 2 selects the 386 LongInt paths |
| 04FE | `FileMode` | u8 | 2 | Read/write |
| 0500 | CR LF | 2 bytes | 0Dh 0Ah | used by `WriteLn` |

Uninitialised globals occupy DS:0502..9DF1 and are zeroed at start-up (`3A9A:1A76`). The game's own variables live there, so **every game global starts at 0** unless the game sets it.

### 3.3 Real48 format

Six bytes, little-endian. In registers a Real48 is held as `DX:BX:AX`: AL is the exponent, AH the lowest mantissa byte, BX the middle word and DX the high word.

| Byte | Bits | Meaning |
|---:|---|---|
| 0 | 7..0 | Exponent `e`, bias 129. `e = 0` means the value is exactly 0 (other bytes ignored) |
| 1–4 | — | Mantissa bits 31..0 of the 39-bit fraction (byte 1 least significant) |
| 5 | 6..0 | Mantissa bits 38..32 (most significant) |
| 5 | 7 | Sign (1 = negative) |

Value = (−1)^sign × 2^(e−129) × (1 + fraction/2³⁹). The leading 1 is implicit. Examples: 1.0 = `81 00 00 00 00 00`; 0.5 = `80 00 00 00 00 00`; π/2 = `81 21 A2 DA 0F 49` (AL=81h, AH=21h, BX=DAA2h, DX=490Fh; constant in `BP_RealSin`); −π/2 has DX=C90Fh.

### 3.4 Constants

| Constant | Value | Unit | Provenance |
|---|---|---|---|
| LCG multiplier | `0x08088405` | — | `file0001:3A9A:1833` (code), `3A9A:1869` (low word) |
| LCG increment | 1 | — | `file0001:3A9A:185B` |
| `RandSeed` initial | 0 | — | DGROUP initialised data, DS:04EE (file offset 19136 + 3C43h×16 + 4EEh) |
| `Randomize` source | DOS time: CX = hour:minute, DX = second:hundredths | — | `file0001:3A9A:186B` (never called) |
| PIT input clock used by `Sound` | 1234DDh (1,193,181 Hz) | Hz | `file0001:3A1C:02D4` |

## 4. Behaviour

### 4.1 Random-number generator

The generator has one 32-bit state, `RandSeed`, shared by all callers.

```text
step():                                  # BP_RandNext, file0001:3A9A:1833
  RandSeed := (RandSeed * 0x08088405 + 1) mod 2^32
  return RandSeed

Random(Range: u16) -> u16:               # BP_RandomInt, 3A9A:17D6
  s := step()                            # the seed advances even when Range = 0
  return (s * Range) >> 32               # 32x16 unsigned product, keep bits 32..47
                                         # = floor(s * Range / 2^32); always < Range when Range > 0

Random -> Real48:                        # BP_RandomReal, 3A9A:17F3
  s := step()
  return s / 2^32 as Real48              # exact: 32 significant bits fit the 39-bit mantissa
                                         # (s = 0 gives Real 0.0)

Randomize:                               # BP_Randomize, 3A9A:186B. NOT CALLED by file0001
  RandSeed := (DX << 16) | CX            # from DOS Get Time: CH=hour CL=minute DH=second DL=1/100
```

- `Random(Range)` always uses the **new** seed (the step comes first).
- The scaling is a full 32×16 multiply: `floor(seed × Range / 2³²)`. It is *not* `seed mod Range`, and *not* `(seed >> 16) × Range >> 16` (that variant differs whenever the low word matters). It is unbiased only in the usual LCG sense, and the low bits of the seed have little influence.
- `Range` is a Word. A negative Integer argument is reinterpreted as unsigned (for example −1 → 65535).
- **Seeding:** because `Randomize` is never called and nothing else writes `RandSeed`, the sequence starts at seed 0 every time an episode program starts. It then advances once per `Random` call, wherever the call is made, so the state carries across missions within one run of the program. A port that wants T1 replays must (a) start the generator at 0 when the episode program would start, (b) make exactly the same calls in the same order, including calls outside gameplay proper (the sound-effect unit calls `Random`, §8 Q2), and (c) never add RNG calls of its own to the shared stream.

Call sites in `file0001` (for M8-03 and M3-06): `Random(Range)` at `051A:6A43`; `0E8B:1500`, `1527`, `1712`, `1732`, `9C10`, `9C1F`, `9C89`, `9E5B`, `9F20`, `9F2F`, `A27B`, `A2B0`, `A2BF`, `A2CE`, `A2E1`, `A448`; `1E2A:0390`, `048A`, `04C5`, `0511`, `0560`, `05B3`; `34E6:0272`. Real `Random` at `34E6:0A72`.

### 4.2 Real48 arithmetic used by game code

Game code (not just the runtime) calls these System entry points. The register convention is: first operand in `DX:BX:AX`, second in `DI:SI:CX`, result in `DX:BX:AX` (Real) or `DX:AX` (LongInt), and comparison results in the flags.

| Entry | Operation | Game call sites | Error behaviour | Semantics known |
|---|---|---:|---|---|
| `3A9A:1317` | `X + Y` | 3 | RunError 205 on overflow | [real48 §4.1](real48.md#41-core-add-and-subtract-3a9a1006-subtract-3a9a1002): 48-bit field, truncating alignment |
| `3A9A:1329` | `X * Y` | 6 | 205 on overflow | [real48 §4.2](real48.md#42-core-multiply-3a9a10c9): correctly rounded, ties away |
| `3A9A:132F` | `X / Y` | 128 | 200 if Y = 0; 205 on overflow | [real48 §4.3](real48.md#43-core-divide-3a9a11cc-far-entry-3a9a132f): correctly rounded, ties away |
| `3A9A:1339` | compare `X ? Y` | 20 | — | If the sign bits differ, the negative one is smaller. Otherwise exponent, then mantissa, inverted for negatives. Equal when both exponents are 0 |
| `3A9A:133D` | LongInt → Real | 160 | — | **Exact** (normalise the magnitude, set the sign) |
| `3A9A:1349` | `Round(X)` → LongInt | 141 | 207 if out of LongInt range | Round **half away from zero**. Only the first discarded bit decides; \|X\| ≥ 2³¹ (except −2³¹) is an error |
| `3A9A:13F0` | `Sqrt(X)` | 5 | 207 if X < 0 | Newton iteration from a halved-exponent guess ([real48 §4.9](real48.md#49-sqrt-3a9a13f0)) |
| `3A9A:1462` | `Sin(X)` | 1 | — | Reduction by 2π, fold to [0, π/2], degree-15 odd polynomial ([real48 §4.10](real48.md#410-sin-3a9a1462-and-cos-3a9a144f)) |
| `3A9A:144F` | `Cos(X)` | 1 | — | Computed as `Sin(π/2 − X)` |
| `3A9A:1639` | `ArcTan(X)` | 4 | — | \|X\| ≥ 1 reduced via 1/X, tangent table, degree-11 odd polynomial ([real48 §4.11](real48.md#411-arctan-3a9a1639)) |

Entries present but not referenced by game code: `X − Y` (`3A9A:131D`), `Sqr` (`3A9A:1323`), `Trunc` (`3A9A:1341`, the same core as `Round` without the rounding bit), `Int`/`Frac` (used only inside `Sin`). Unit `0E8B` makes most of the calls: `0E8B:B880` alone has 55 divide, 55 LongInt→Real and 55 `Round` calls (the pattern `Round(Real(a) / b)`), and `0E8B:82DD` uses `Sin`, `Cos`, `ArcTan`, `Sqrt`, multiply and compare (probably heading or aiming maths). Units `0000`, `051A`, `1E2A`, `2404`, `3292` and `34E6` also convert to and from Real48, and most of them divide. Because results feed game state, **Classic gameplay must use a bit-exact Real48 implementation, not `f32`/`f64`** (AGENTS.md rule 6; MDG R11).

### 4.3 Integer semantics

- No range or overflow checking anywhere: the System error entries for range (201) and overflow (215) are never called. 16-bit and 32-bit arithmetic wraps, and out-of-range array indexes read or write neighbouring memory. Wrap explicitly in the port wherever the original can wrap (AGENTS.md rule 6).
- LongInt multiply (`3A9A:0BEF`, 47 game calls) and divide/modulo (`3A9A:0C2C`) use 32-bit instructions when `Test8086 ≥ 2` and a 16-bit routine otherwise. Both paths are expected to give the standard two's-complement results: the product is the low 32 bits, and division truncates toward zero with the remainder taking the dividend's sign (LIKELY; the 16-bit path was not traced instruction by instruction). Division by zero is RunError 200.
- Stack checking (`{$S+}`) is on for most Pascal procedures (`3A9A:0530`, 130 callers). I/O checking (`{$I+}`) is on (`3A9A:04F4`, 477 calls): a failed `Reset`/`BlockRead` aborts with a runtime error unless the code checks `IOResult` (`3A9A:04ED`) itself. That matters only for the port's error policy, not for game state.

### 4.4 Strings

ShortStrings are a length byte followed by up to 255 characters. String assignment (`3A9A:0D1E`, 987 calls) copies `min(length(Src), MaxLen)` characters, where `MaxLen` is the declared size of the destination: **assignments silently truncate**. Concatenation (`3A9A:0D83`) appends into a 255-character temporary, and the total is capped at 255 (LIKELY). Comparison (`3A9A:0DF5`) is a byte-wise comparison of the characters, then of the lengths (LIKELY; standard BP7). `UpCase` (`3A9A:1A63`) maps only `a..z`.

### 4.5 Crt and Dos (what the game uses)

| Routine | Address | Behaviour the port must match or replace |
|---|---|---|
| `Delay(ms)` | `3A1C:02A8` | Busy-waits `ms × DelayCnt` loop iterations. DelayCnt (DS:9B96) is calibrated at start-up against the BIOS tick, so on a correctly calibrated machine `Delay(n)` ≈ n ms. Game calls: 30 (main 4, `051A` 14, `0E8B` 12). See [timing](timing.md) |
| `KeyPressed` / `ReadKey` | `3A1C:0308` / `031A` | BIOS keyboard (INT 16h). An extended key returns `#0` first, then the scan code on the next call. Used by menus and screens (79 call sites). Flight input comes from the game's own INT 9 handler instead ([hardware-access](../../re/hardware-access.md)) |
| `Sound(hz)` / `NoSound` | `3A1C:02D4` / `0301` | PIT channel 2 square wave, divisor `1234DDh div hz`. `hz ≤ 18` is ignored. Used for PC-speaker effects (`0000`, `1E2A`, `0E8B`) |
| `TextMode`, `ClrScr`, `GotoXY`, `TextColor`, `TextBackground` | `3A1C:0177`..`027D` | Text-mode screens in the main program only (config and exit messages). Not ported as such |
| `GetTime` | `3A7E:0036` | DOS time. Called 5 times (`051A`, `0E8B`): see [timing](timing.md) |
| `GetIntVec` / `SetIntVec` | `3A7E:0072` / `008A` | Interrupt hooks (`0E8B`: keyboard; `1E2A`) |
| `Exec` | `3A7E:009D` | Called twice from `0E8B`. This is the `label.exe` path: **never port** (AGENTS.md rule 10) |

## 5. Edge cases and original bugs

- **Fixed RNG seed.** Every run of an episode program replays the same random stream from seed 0. This is original behaviour that the port must **preserve** in Classic (it is what makes T1 replays possible). Enhanced may offer a seeded or randomised start, but only as an explicit setting.
- `Random(0)` returns 0 and still advances the seed.
- `Round(x)` for exact halves goes away from zero (2.5 → 3, −2.5 → −3), unlike banker's rounding in many modern libraries. Do not use `f64::round_ties_even`; and do not use `f64::round` either, because the input is a Real48.
- Silent integer wrap and truncating string assignment (§4.3, §4.4) are preserved wherever they influence state.
- Runtime errors (heap, I/O, Real overflow, divide by zero) terminate the original with "Runtime error N at …". The port reports a fatal error instead (T3).

## 6. Test vectors

Derived statically. The formula and a register-level model of the routine's instruction sequence agree on 200,000 random seeds. Two independent lines of static evidence make this CONFIRMED. A DOSBox RandSeed watch (§8 Q1) would add a dynamic line.

| # | Setup / input | Expected output | Evidence |
|---|---|---|---|
| 1 | Seed sequence from `RandSeed = 0` (values after each step) | `00000001`, `08088406`, `DC6DAC1F`, `33DC589C`, `45DE2B0D`, `ABF18B42`, `5195C04B`, `296B6D78`, `5F4B0359`, `6CF8F4BE` | static |
| 2 | 10 × `Random(100)` from seed 0 | 0, 3, 86, 20, 27, 67, 31, 16, 37, 42 | static |
| 3 | 10 × `Random(320)` from seed 0 | 0, 10, 275, 64, 87, 214, 101, 51, 119, 136 | static |
| 4 | 10 × `Random(2)` from seed 0 | 0, 0, 1, 0, 0, 1, 0, 0, 0, 0 | static |
| 5 | 10 × `Random(65535)` from seed 0 | 0, 2056, 56428, 13276, 17885, 44016, 20885, 10603, 24394, 27896 | static |
| 6 | `Random(10)`, `Random(100)`, `Random(320)`, `Random(2)`, `Random(65535)`, `Random(7)` from seed 0 | 0, 3, 275, 0, 17885, 4; final seed `ABF18B42` | static |
| 7 | 10 × `Random(0)` from seed 0 | ten 0s; final seed `6CF8F4BE` (same as #1) | static |
| 8 | Real `Random` × 3 from seed 0 (Real48 bytes, exponent first) | `61 00 00 00 00 00` (2⁻³²), `7C 00 60 40 88 00` (0.0313799395…), `80 00 1F AC 6D 5C` (0.8610484672…) | static |
| 9 | `Randomize` at 13:45:30.25 (not used by the game) | RandSeed = `1E190D2D`; then 5 × `Random(100)` = 32, 34, 34, 25, 80 | static |
| 10 | LongInt → Real: 1, −1, 65536 | `81 00 00 00 00 00`, `81 00 00 00 00 80`, `91 00 00 00 00 00` | static (`3A9A:126D`) |
| 11 | `Round`: 0.5, 1.5, 2.5, −2.5, 0.4999… (largest Real48 below 0.5) | 1, 2, 3, −3, 0 | static (`3A9A:12A9`) |

## 7. Presentation and audio hooks

None directly. The `Random` calls in the sound-effect unit (`1E2A`) mean that SFX choices draw from the same stream as gameplay (§8 Q2).

## 8. Open questions

- [ ] Q1 (VE): confirm dynamically that RandSeed (DS:04EE in DGROUP) is 0 at the start of `file0001` and follows test vector 1 call by call. Record the number of `Random` calls between program start and the first in-flight tick of mission 1, so replays can be aligned.
- [ ] Q2 (U-030): are the `Random` calls in `1E2A` (sound effects) made even when sound is disabled (`/nosound`, no card)? If they depend on the sound configuration, the RNG stream, and so gameplay, differs between sound settings.
- [x] Q3 (U-029, P1), resolved 2026-11-02 in [real48.md](real48.md): bit-exact Real48 add, multiply and divide (rounding of the 40-bit mantissa, normalisation, underflow-to-zero) and the exact Sqrt/Sin/Cos/ArcTan algorithms and coefficient tables (`3A9A:14C3`, `3A9A:174A`). Needed before any `td-sim` code that mirrors `0E8B:B880` or `0E8B:82DD`. The best method is to run the original routines on many inputs in an emulator harness (for example the DOSBox oracle, or a small 8086 interpreter over the System segment bytes in `re-work`) and specify from the observed bit patterns plus the code.
- [ ] Q4 (U-031): roles of units `2404`, `3292`, `34E6` and the small unit `3A92` (M1-04/M1-07).

## 9. Provenance

All addresses are `file0001:` image-relative. Ghidra address = (1000h + seg):off.

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| LCG step `×0x08088405 + 1`, 32-bit | `3A9A:1833`–`1868`; constant `3A9A:1869` | static (code read + register model) | CONFIRMED |
| `Random(Range) = (seed × Range) >> 32` with the new seed | `3A9A:17D6` | static | CONFIRMED |
| Real `Random = seed / 2³²` | `3A9A:17F3` | static | CONFIRMED |
| `Randomize` uses DOS time CX:DX; no callers | `3A9A:186B` (no far call targets it) | static (relocation scan + code) | CONFIRMED |
| RandSeed at DS:04EE, initial 0, written only by the LCG step | DGROUP `3C43:04EE`; raw byte scan for `[04EE]`/`[04F0]` operands | static | CONFIRMED (dynamic check pending, Q1) |
| Game calls Real48 helpers (≈ 470 call sites) | call graph from `3A9A:1317`..`1639` | static | CONFIRMED |
| Round half away from zero; LongInt→Real exact | `3A9A:12A9`, `3A9A:126D` | static | LIKELY |
| Compiled `{$R-,Q-}`, `{$S+}`, `{$I+}` | `3A9A:0524`/`052A` unreferenced; `3A9A:0530`/`04F4` callers | static | CONFIRMED |
| Unit map and init order | entry `0000:4526` init chain; relocation segment scan | static | CONFIRMED (roles HYPOTHESIS/LIKELY as marked) |
| System typed-constant layout | System init stores (`3A9A:0000`..); DGROUP bytes | static | CONFIRMED |
| `Sound` divisor constant 1234DDh | `3A1C:02D4` | static | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-1 (Archaeologist + Spec author) | Initial: RNG exact, Real48 inventory, unit map, System constants |
| 2 | 2026-11-02 | RE-4 (Archaeologist + Spec author) | Real48 algorithms moved to the new [real48.md](real48.md) spec; §4.2 links to it; Q3 resolved |
