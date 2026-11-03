# Real48 arithmetic (bit-exact)

| | |
|---|---|
| **Spec ID** | `core/real48` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (gameplay state is computed with these operations; results must match bit for bit) |
| **Confidence** | CONFIRMED: two independent lines of evidence. (1) The original System routines run on an 8086 interpreter; (2) an independent model written from this spec. They agree on every one of 247,705 inputs (§6.1) |
| **Tasks** | M1-01.b (feeds M6-02 `td-sim` Real48 type, M6-01) |
| **Unknowns** | U-029 (resolved by this spec) |

## 1. Summary

The episode programs do their floating-point maths with Borland Pascal 7's software `Real` type, a 6-byte format called Real48 here. Game code calls ten System entry points: add, multiply, divide, compare, LongInt→Real, `Round`, `Sqrt`, `Sin`, `Cos` and `ArcTan` (inventory and call counts in [bp7-runtime-semantics §4.2](bp7-runtime-semantics.md#42-real48-arithmetic-used-by-game-code)). This spec gives each one exactly: the working precision, alignment, normalisation, rounding, underflow and overflow, and for the transcendental functions the argument reduction, the polynomials, their coefficients as exact Real48 bytes, and the evaluation order.

The short version:

- **Multiply and divide** are correctly rounded to nearest, ties away from zero.
- **Add and subtract** work in a 48-bit field (40 significand bits plus 8 guard bits). The smaller operand is aligned by a truncating shift with no sticky bit, so a subtraction can come out one unit in the last place (ulp) above the correctly rounded result. This happens in about 0.1% of random cases.
- **Underflow** gives +0 silently. **Overflow** is RunError 205 at the game-visible entry points.
- **`Sqrt`** is a Newton iteration with a data-dependent stopping rule. **`Sin`/`Cos`** reduce by 2π, fold into [0, π/2] and evaluate a degree-15 odd polynomial. **`ArcTan`** reduces with 1/x and a three-entry tangent table and evaluates a degree-11 odd polynomial. All of these are built from the core operations above, so they inherit their rounding.

A port must implement these algorithms on integers. Host floating point (`f32`/`f64`) is not acceptable for Classic gameplay (AGENTS.md rule 6).

## 2. Definitions and units

| Term | Meaning |
|---|---|
| Real48 `r` | 6 bytes `r[0]..r[5]` in memory order. Layout: [bp7-runtime-semantics §3.3](bp7-runtime-semantics.md#33-real48-format). In registers: `DX:BX:AX` (first operand, result) and `DI:SI:CX` (second operand) |
| `e(r)` | Exponent byte `r[0]`, 0..255, bias 129 |
| `s(r)` | Sign bit, bit 7 of `r[5]` |
| `m(r)` | 40-bit significand with the hidden bit: `2^39 + (r[1] + 2^8·r[2] + 2^16·r[3] + 2^24·r[4] + 2^32·(r[5] & 0x7F))`, so `2^39 ≤ m < 2^40` |
| value | `e = 0`: zero, whatever the other five bytes hold. Otherwise `(−1)^s × m × 2^(e − 168)` |
| canonical zero | All six bytes `00`. Every computed zero result is canonical (positive sign) unless a rule below says that an operand is returned unchanged |
| `pack(s, e, m)` | Inverse of the above, for `1 ≤ e ≤ 255` and `2^39 ≤ m < 2^40` |
| working field `W` | A 48-bit unsigned integer that holds `m × 2^8`: 40 significand bits over 8 guard bits |
| overflow | The core routine reports overflow (carry set). The game-visible entry points turn it into **RunError 205**. Internal callers (inside `Sqrt`, `Sin`, `Cos`, `ArcTan`) ignore it |
| `round48(W)` | `W ← W + 128`; if that reaches `2^48`, then `W ← 2^47` and the exponent goes up by 1. The result significand is `floor(W / 2^8)`. This is round half up on magnitude, using only the bits that are still in `W` |

All quantities in the pseudocode are mathematical integers. Where the original works in 8- or 16-bit registers, the effect of the wrap is written out explicitly.

## 3. Data

### 3.1 Constants (exact bytes, memory order)

| Name | Bytes | Value ≈ | Used by | Provenance (`file0001:`) |
|---|---|---|---|---|
| `ONE` | `81 00 00 00 00 00` | 1 | polynomials, ArcTan | `3A9A:17C8`, `3A9A:1652`, `3A9A:16C9` (immediates) |
| `HALF_PI` | `81 21 A2 DA 0F 49` | 1.570796326794152 | ArcTan | `3A9A:1701` |
| `NEG_HALF_PI` | `81 21 A2 DA 0F C9` | −1.570796326794152 | Cos | `3A9A:144F` |
| `PI` | `82 21 A2 DA 0F 49` | 3.141592653588305 | Sin | derived in place from `TWO_PI` (exponent − 1), `3A9A:1491` |
| `HALF_PI` (Sin) | `81 21 A2 DA 0F 49` | 1.570796326794152 | Sin | exponent − 1 again, `3A9A:149C` |
| `TWO_PI` | `83 21 A2 DA 0F 49` | 6.283185307176609 | Sin | `3A9A:1466` |
| `ATAN_SMALL` | `7E 4A 8E E9 6F 0C` | 0.137145661649356 | ArcTan | `3A9A:166A` |

### 3.2 Sine polynomial coefficients `SIN_C[0..6]` (`file0001:3A9A:14C3`, 7 × 6 bytes)

Listed in evaluation order (the highest power first). They are the Taylor coefficients ±1/(2k+1)! rounded to Real48.

| i | Bytes | Value ≈ | Nominal |
|---:|---|---|---|
| 0 | `58 9D 39 9F 3F D7` | −7.647163731813581e−13 | −1/15! |
| 1 | `60 43 9D 30 92 30` | 1.605904383681299e−10 | 1/13! |
| 2 | `67 AA 3F 28 32 D7` | −2.505210305636706e−08 | −1/11! |
| 3 | `6E B6 2A 1D EF 38` | 2.755731922397808e−06 | 1/9! |
| 4 | `74 0D D0 00 0D D0` | −1.984126984126977e−04 | −1/7! |
| 5 | `7A 88 88 88 88 08` | 8.333333333325754e−03 | 1/5! |
| 6 | `7E AB AA AA AA AA` | −1.666666666667425e−01 | −1/3! |

### 3.3 Arctangent polynomial coefficients `ATAN_C[0..4]` (`file0001:3A9A:174A`, 5 × 6 bytes)

| i | Bytes | Value ≈ | Nominal |
|---:|---|---|---|
| 0 | `7D E8 A2 8B 2E BA` | −0.09090909090900823 | −1/11 |
| 1 | `7D 8E E3 38 8E 63` | 0.1111111111110858 | 1/9 |
| 2 | `7E 49 92 24 49 92` | −0.1428571428571104 | −1/7 |
| 3 | `7E CD CC CC CC 4C` | 0.2000000000000455 | 1/5 |
| 4 | `7F AB AA AA AA AA` | −0.3333333333334849 | −1/3 |

### 3.4 Arctangent reduction table (`file0001:3A9A:171A`, 18 bytes per row)

| Row | Upper bound (exclusive) | `t` = tangent of the centre | `a` = centre angle |
|---:|---|---|---|
| 0 | `7F E7 CF CC 13 54` ≈ 0.4142135623728791 (tan π/8) | `7F F6 F4 A2 30 09` ≈ 0.2679491924309332 (tan π/12) | `7F 6A C1 91 0A 06` ≈ 0.261799387798419 (π/12) |
| 1 | `80 B5 9E 8A 6F 44` ≈ 0.7673269879787767 (tan 5π/24) | `80 82 2C 3A CD 13` ≈ 0.5773502691899921 (tan π/6) | `80 6A C1 91 0A 06` ≈ 0.5235987755968381 (π/6) |
| 2 | (none; everything up to 1) | `81 00 00 00 00 00` = 1 | `80 21 A2 DA 0F 49` ≈ 0.7853981633970761 (π/4) |

## 4. Behaviour

### 4.1 Core: add and subtract (`3A9A:1006`; subtract `3A9A:1002`)

`sub(a, b)` is `add(a, b′)`, where `b′` is `b` with its sign bit flipped (also when `b` is zero).

```text
add(a, b):
  if e(b) = 0: return a unchanged (all six bytes)
  if e(a) = 0: return b unchanged
  # R = operand with the larger exponent, T = the other one; on a tie R = b
  if e(a) > e(b): R, T := a, b  else: R, T := b, a
  k := e(R) − e(T)
  if k ≥ 41: return R unchanged
  WR := m(R) · 2^8
  WT := floor(m(T) · 2^8 / 2^k)            # truncating alignment: bits shifted out are lost
  e  := e(R)
  if s(R) = s(T):
      W := WR + WT;  s := s(R)
      if W ≥ 2^48:  W := floor(W / 2);  e := e + 1;  if e = 256: overflow
  else:
      D := WT − WR
      if D = 0: return canonical zero
      if D < 0: W := −D;  s := s(R)  else: W := D;  s := s(T)
      while W < 2^47:                       # normalise to the left, shifting in zeros
          W := 2W;  e := e − 1
          if e = 0: return canonical zero   # underflow, no error
  round48(W)  (if it carries: e := e + 1; if e = 256: overflow)
  return pack(s, e, floor(W / 2^8))
```

Consequences:

- With equal signs the result is the correctly rounded sum (ties away from zero). Truncating `WT` cannot change the first guard bit of a sum.
- With opposite signs and `k ≥ 9`, `WT` may have lost bits, and `WR − WT` is then the **ceiling** of the exact difference in units of the field. When the exact difference sits just below a rounding boundary, the result is 1 ulp above the correctly rounded value. Vector 8 in §6 is an example. A port must use this exact procedure, not a correctly rounded subtraction.
- Exponent differences of 0 to 8 are exact before rounding. A difference of 40 can still change the last bit (vectors 5 and 7). From 41 on, the larger operand comes back unchanged.
- The result is the same for `add(a, b)` and `add(b, a)`.

### 4.2 Core: multiply (`3A9A:10C9`)

```text
mul(a, b):
  if e(a) = 0 or e(b) = 0: return canonical zero
  P := floor(m(a) · m(b) / 2^32)            # 47 or 48 bits; the low 32 bits of the exact product are dropped
  e := e(a) + e(b) − 128                    # plain integer, may be ≤ 0 or ≥ 256 at this point
  if P < 2^47:  P := 2P;  e := e − 1
  P := P + 128
  if P ≥ 2^48:  P := 2^47;  e := e + 1
  if e ≤ 0:   return canonical zero         # underflow, no error
  if e ≥ 256: overflow
  return pack(s(a) xor s(b), e, floor(P / 2^8))
```

This is the correctly rounded product (ties away from zero). Dropping the low bits never affects the deciding bit. The original has a shorter instruction path for operands whose low 24 significand bits are zero; it gives the same result.

### 4.3 Core: divide (`3A9A:11CC`; far entry `3A9A:132F`)

```text
div(a, b):                                  # the far entry first raises RunError 200 if e(b) = 0
  if e(a) = 0: return canonical zero
  Q := floor(m(a) · 2^41 / m(b))            # 42 quotient bits, exact restoring division
  W := Q · 2^6 + 63                         # the 6 bits below the quotient read as ones
  e := e(a) − e(b) + 129
  if W < 2^47:  W := (2W) mod 2^48;  e := e − 1
  W := W + 128
  if W ≥ 2^48:  W := 2^47;  e := e + 1
  if e ≤ 0:   return canonical zero         # underflow, no error
  if e ≥ 256: overflow
  return pack(s(a) xor s(b), e, floor(W / 2^8))
```

The round-up decision is the first quotient bit after the 40 significand bits, and the trailing ones cannot carry. The result is therefore the correctly rounded quotient, ties away from zero. The core does not check `e(b)`; only the far entry does. The internal callers never divide by zero.

### 4.4 Compare (`3A9A:1243`; far entry `3A9A:1339`)

The result is one of *less*, *equal*, *greater* for `a ? b`. (In the original: carry set = less, zero flag = equal. Game code branches with unsigned conditions.)

```text
cmp(a, b):
  if s(a) ≠ s(b): return less if s(a) = 1 else greater      # sign bits decide first, even for zeros
  if e(a) = 0 and e(b) = 0: return equal                     # other bytes ignored
  key(x) := (r[0], r[5], r[4], r[3], r[2], r[1]) as unsigned bytes, compared lexicographically
  if key(a) = key(b): return equal
  order := less if key(a) < key(b) else greater
  return order if s(a) = 0 else the reverse of order
```

A zero with the sign bit set ("−0") compares **less** than +0 and is not equal to it. Unary minus on a `Real` is done inline by flipping the sign bit, so `−x` with `x = 0` produces exactly that pattern. A zero compared with a nonzero value of the same sign is smaller in magnitude, as expected.

### 4.5 LongInt → Real (`3A9A:126D`; far entry `3A9A:133D`)

Exact. 0 gives canonical zero. Otherwise `s` = sign of the LongInt, `n` = bit length of `|v|` (1..32; `|−2^31| = 2^31`), `e = 128 + n`, `m = |v| · 2^(40 − n)`.

### 4.6 Real → LongInt: `Round` (`3A9A:1349`) and `Trunc` (`3A9A:1341`, not called by the game)

```text
to_long(x, rounding):
  if e(x) > 160: RunError 207
  if e(x) < 128: return 0                               # |x| < 0.5, and every zero
  I := floor(m · 2^(e − 168))                          # magnitude of the integer part (0 when e = 128)
  f := bit (167 − e) of m                              # first fractional bit (the hidden bit when e = 128)
  if rounding and f = 1: I := I + 1;  if I ≥ 2^32: RunError 207
  if I = 0: return 0
  v := (s = 1) ? −I : I, as 32-bit two's complement
  if bit 31 of v ≠ s: RunError 207                      # |result| too large; −2^31 itself is fine
  return v
```

`Round` is half away from zero, and only the first fractional bit decides (bp7-runtime-semantics §5).

### 4.7 `Int` and `Frac` (internal; used only by `Sin`)

- `int(x)`: if `e ≥ 168`, return `x` unchanged. If `e ≤ 128`, return `x` with the exponent byte set to 0 (a zero; the other bytes are kept). Otherwise clear the low `168 − e` bits of the 40-bit field held in bytes 1..5. The sign bit is never cleared.
- `frac(x) := sub(x, int(x))`, using the core (no error check). So `frac(x) = x` when `|x| < 1`.

### 4.8 Odd polynomial helper (`3A9A:176E`, Horner core `3A9A:1787`)

```text
poly_odd(x, C[0..n−1]):                     # x · (1 + y·(C[n−1] + y·(C[n−2] + … + y·C[0])))
  y   := mul(x, x)
  acc := mul(C[0], y)
  for i in 1 .. n−1:
      acc := add(acc, C[i])
      acc := mul(acc, y)
  acc := add(acc, ONE)
  return mul(acc, x)
```

All steps use the core routines (overflow ignored). The order of operations matters, because every step rounds. Operand order within one `add` or `mul` does not.

### 4.9 `Sqrt` (`3A9A:13F0`)

```text
sqrt(x):
  if e(x) = 0: return x unchanged
  if s(x) = 1: RunError 207
  eg    := 128 + floor((e(x) − 128) / 2)       # floor toward −∞; 1..255 maps to 64..191
  g     := x with its exponent byte replaced by eg   # the first guess keeps x's significand bits
  limit := eg − 20
  loop:
      q := div(x, g)
      n := add(q, g)
      n := n with its exponent byte decreased by 1   # halve (n is never zero here)
      d := sub(n, g)
      g := n
      if e(d) < limit: return g                # e(d) is the raw exponent byte; d = 0 counts as 0
```

The loop stops once a step changes the estimate by less than about 2^−20 of the first guess. That is typically one or two iterations after convergence. The final estimate is often not the correctly rounded root: about a quarter of random inputs end 1 ulp away from it. So the procedure is normative, not any rounding property.

### 4.10 `Sin` (`3A9A:1462`) and `Cos` (`3A9A:144F`)

```text
sin(x):
  if e(x) < 108: return x unchanged                    # |x| < 2^−21
  if |x| ≥ TWO_PI (compare with the sign bit cleared):
      x := mul(frac(div(x, TWO_PI)), TWO_PI)           # keeps the sign of x
  if s(x) = 1: x := add(x, TWO_PI)                     # now 0 ≤ x < 2π
  upper := not (x < PI)
  if upper: x := sub(x, PI)
  if not (x < HALF_PI): x := add(−x, PI)               # π − x, with −x = x with the sign bit set
  if e(x) ≥ 108: x := poly_odd(x, SIN_C)               # 7 coefficients, §3.2
  if upper and e(x) ≠ 0: flip the sign bit of x
  return x

cos(x):
  y := add(x, NEG_HALF_PI)                             # x − π/2
  if e(y) ≠ 0: flip the sign bit of y                   # π/2 − x
  return sin(y)
```

- The comparisons are §4.4, and the arithmetic uses the core routines (no error checks).
- For |x| ≥ 2^39 or so, `div(x, TWO_PI)` has no fractional bits left, `frac` gives 0 and the result is 0. Arguments in that range are meaningless anyway.
- **`Cos(0)` is not 1**: it is `sin(HALF_PI)` = `80 FA FF FF FF 7F` (0.999999999995). Likewise `Sin(HALF_PI)`. `Sin(PI)` and `Cos(HALF_PI)` are exactly 0.

### 4.11 `ArcTan` (`3A9A:1639`)

```text
arctan(x):
  if e(x) = 0: return x unchanged
  negative := (s(x) = 1);  x := |x|
  inverted := false
  if not (x < ONE): x := div(ONE, x);  inverted := true     # x = 1 also inverts
  if x < ATAN_SMALL:
      r := poly_odd(x, ATAN_C)                              # 5 coefficients, §3.3
  else:
      pick the first row of §3.4 whose bound is > x (row 2 if none)
      num := sub(x, t)
      den := add(mul(x, t), ONE)
      u   := div(num, den)                                  # tan(θ − centre), |u| ≤ about 0.132
      r   := add(poly_odd(u, ATAN_C), a)
  if inverted: r := sub(HALF_PI, r)
  if negative: set the sign bit of r
  return r
```

Evaluation order: `num` first, then `x·t`, then `+1`, then the division, then the polynomial, then `+ a`.

### 4.12 Error behaviour at the game-visible entry points

| Entry | Error |
|---|---|
| add `3A9A:1317`, sub `3A9A:131D`, mul `3A9A:1329` | RunError 205 on overflow |
| div `3A9A:132F` | RunError 200 if `e(divisor) = 0` (checked first; `0/0` is 200); 205 on overflow |
| `Round` `3A9A:1349`, `Trunc` `3A9A:1341` | RunError 207 when out of LongInt range (§4.6) |
| `Sqrt` `3A9A:13F0` | RunError 207 for a negative nonzero argument |
| compare, LongInt→Real, `Sin`, `Cos`, `ArcTan` | none |

Underflow is never an error: results with exponent ≤ 0 become canonical zero. RunErrors end the original program. The port reports a fatal error ([bp7-runtime-semantics §5](bp7-runtime-semantics.md#5-edge-cases-and-original-bugs)).

## 5. Edge cases and original bugs

| Quirk | Original behaviour | Port policy |
|---|---|---|
| Subtraction not correctly rounded | Up to 1 ulp high when the exponents differ by ≥ 9 and the aligned operand lost bits (§4.1) | **Preserve** (T1) |
| −0 | `cmp(−0, +0)` = less; `−0 = 0` is false | **Preserve**. Keep the sign bit when negating zero |
| Non-canonical zero | Exponent 0 with other bytes nonzero is zero for every operation. `add` returns its other operand unchanged; the bytes survive in `add(x, 0)` = `x` | Preserve byte-exact results (they can reach save files or `state_hash`) |
| `Cos(0)`, `Sin(π/2)` ≠ 1 | `80 FA FF FF FF 7F` | Preserve |
| Tiny arguments | `Sin(x) = x` for `e(x) < 108` (\|x\| < 2^−21). `ArcTan(x)` returns `x` for tiny x because the polynomial rounds to `x·1`. `Cos(x)` for tiny x is `sin(π/2 − x)` | Preserve |
| Huge `Sin`/`Cos` arguments | 0 once `x/2π` has no fractional bits | Preserve |
| Overflow inside `Sqrt`/`Sin`/`Cos`/`ArcTan` | Ignored (never seen in testing) | Treat as unreachable; assert in debug builds |

## 6. Test vectors

### 6.1 Agreement and the full vector file

Method: [`file0001.exe`](../../re/EXECUTABLES.md) was loaded with its relocations into a register-level 8086 interpreter written for this task (scratch tool in `re-work`, not committed). The far entry points were called with operands in `DX:BX:AX`/`DI:SI:CX`, and the result registers or the flags were read back. An independent model written from §4 was run on the same inputs.

| Run | Inputs | Agreement |
|---|---:|---:|
| Smoke test (seed 1) | 6,000 | 100% |
| Two seeds × 10,000 per operation × 12 operations | 240,000 | 100% |
| Vector file generation (every row checked) | 1,705 | 100% |
| **Total** | **247,705** | **100%** |

The input mix includes uniform random bytes, exponent differences −45..45, near-cancellations (one flipped bit), significands with only 15 significant bits (the short multiply path), all-ones and all-zero significands, zeros with junk bytes, exact halves for `Round`, angles in ±8, ±10⁴ and up to 2^47, and the whole exponent range for `Sqrt`.

The **full vector set is [`real48-vectors.csv`](real48-vectors.csv)** (1,705 rows: 185 hand-picked edge cases plus random cases, generated by the original code). Columns: `op` (`add sub mul div cmp from_long round trunc sin cos arctan sqrt`), `a`, `b` (Real48 as 12 hex digits in memory order, exponent byte first, or a decimal LongInt), `result` (Real48 hex, decimal LongInt, `-1`/`0`/`1` for compare, or `E<code>` for a RunError), `class` (`edge` or `random`). `sub` rows are the far subtract entry `3A9A:131D`, which has the same semantics as add with the sign flipped.

### 6.2 Selected vectors

Bytes are in memory order (exponent first). All of them come from the original routines.

| # | Operation | Input(s) | Result | Note |
|---|---|---|---|---|
| 1 | add | `81 00 00 00 00 00` (1) + `82 00 00 00 00 00` (2) | `82 00 00 00 00 40` (3) | |
| 2 | add | `7D CD CC CC CC 4C` (0.1) + `7E CD CC CC CC 4C` (0.2) | `7F 9A 99 99 99 19` | |
| 3 | add | 1 + `81 00 00 00 00 80` (−1) | `00 00 00 00 00 00` | canonical zero |
| 4 | add | `FF FF FF FF FF 7F` + itself | RunError 205 | overflow |
| 5 | add | 1 + `59 FF FF FF FF 7F` (k = 40) | `81 01 00 00 00 00` | the tiny operand still rounds up |
| 6 | add | 1 + `58 FF FF FF FF 7F` (k = 41) | `81 00 00 00 00 00` | larger operand returned unchanged |
| 7 | sub | 1 − `80 FF FF FF FF 7F` (1 − 2^−40) | `59 00 00 00 00 00` (2^−40) | exact cancellation |
| 8 | add | `92 03 9C FC 9F 02` + `9D FE 95 8C 5B BE` | `9D 6B 96 38 4B BE` | correctly rounded would be `9D 6A 96 38 4B BE` (§4.1) |
| 9 | sub | `02 01 00 00 00 00` − `02 00 00 00 00 00` | `00 00 00 00 00 00` | underflow to zero |
| 10 | mul | `82 00 00 00 00 40` (3) × 3 | `84 00 00 00 00 10` (9) | |
| 11 | mul | 0.1 × `84 00 00 00 00 20` (10) | `81 00 00 00 00 00` (1) | |
| 12 | mul | `01 00 00 00 00 00` (MinReal) × `80 00 00 00 00 00` (0.5) | `00 00 00 00 00 00` | underflow |
| 13 | mul | `FF FF FF FF FF 7F` × 2 | RunError 205 | |
| 14 | div | 1 / 3 | `7F AB AA AA AA 2A` | |
| 15 | div | 2 / 3 | `80 AB AA AA AA 2A` | |
| 16 | div | 1 / 0, and 0 / 0 | RunError 200 | |
| 17 | div | `FF FF FF FF FF 7F` / 0.5 | RunError 205 | |
| 18 | cmp | `00 00 00 00 00 80` (−0) ? `00 00 00 00 00 00` | less | |
| 19 | cmp | `00 12 34 56 78 12` ? `00 00 00 00 00 00` | equal | junk bytes ignored |
| 20 | cmp | −1 ? −2 | greater | |
| 21 | Round / Trunc | `9F 00 FF FF FF 7F` (2147483647.5) | RunError 207 / 2147483647 | |
| 22 | Round / Trunc | `A0 80 00 00 00 80` (−2147483648.5) | RunError 207 / −2147483648 | |
| 23 | Sqrt | 2 | `81 FA 33 F3 04 35` | |
| 24 | Sqrt | `84 00 00 00 00 10` (9) | `82 00 00 00 00 40` (3) | |
| 25 | Sqrt | `FF FF FF FF FF 7F` | `C0 FA 33 F3 04 35` | |
| 26 | Sqrt | −1 | RunError 207 | |
| 27 | Sin | 1 | `80 48 78 A4 6A 57` | |
| 28 | Sin | 10 | `80 9B AF F7 44 8B` | reduced by 2π |
| 29 | Sin | `82 21 A2 DA 0F 49` (PI) | `00 00 00 00 00 00` | |
| 30 | Sin | `69 E5 D5 94 BF 56` (≈ 1e−7) | same bytes | tiny argument |
| 31 | Cos | 0 | `80 FA FF FF FF 7F` | not 1 |
| 32 | Cos | 1 | `80 A8 7D 40 51 0A` | |
| 33 | Cos | `81 21 A2 DA 0F 49` (HALF_PI) | `00 00 00 00 00 00` | |
| 34 | ArcTan | 1 | `80 21 A2 DA 0F 49` | the π/4 table constant |
| 35 | ArcTan | 0.5 | `7F 0A 2B 38 63 6D` | |
| 36 | ArcTan | 10 | `81 B0 60 E9 4D 3C` | inverted path |
| 37 | ArcTan | −1 | `80 21 A2 DA 0F C9` | |
| 38 | LongInt→Real | −2147483648 | `A0 00 00 00 00 80` | |

## 7. Presentation and audio hooks

None.

## 8. Open questions

- [ ] Q1 (VE, optional third line of evidence): spot-check a few rows of `real48-vectors.csv` in the DOSBox oracle, by breaking on a game call to `3A9A:132F` and comparing the registers before and after.
- [ ] Q2: `Ln` (`3A9A:14ED`) and `Exp` (`3A9A:1596`) are linked in but not called by game code. If a later finding shows a call, they need the same treatment (coefficient tables at `3A9A:1572` and `3A9A:1609`).
- [x] Q3 (M1-09): E2 and E3 link the same System unit (`td2:350D`, `td3:311C`, identical size). Confirm byte-identity of `:1002..:17D6` before reusing this spec there. **Resolved (RE-7):** System is code-identical in all three episodes apart from DGROUP offsets; this spec applies unchanged ([episode-deltas §3.1](../../re/episode-deltas.md#31-identical-units)).

## 9. Provenance

All addresses are `file0001:` image-relative (Ghidra = `(1000h + seg):off`).

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Add/sub: 48-bit field, truncating alignment, k ≥ 41 shortcut, normalise, round on guard bit 7 | `3A9A:1002..10C8` | static read + interpreter + model agreement | CONFIRMED |
| Multiply: product truncated to the top 48 bits, rounding and exponent checks shared with divide | `3A9A:10C9..11CB` | same | CONFIRMED |
| Divide: 42-bit restoring division with complemented quotient bits | `3A9A:11CC..1242` | same | CONFIRMED |
| Compare | `3A9A:1243..126C` | same | CONFIRMED |
| LongInt→Real, Round/Trunc | `3A9A:126D..1316` | same | CONFIRMED |
| Far entries and RunError codes 200/205/207 | `3A9A:1317..1362`, `3A9A:1446`, `3A9A:14F6` | static | CONFIRMED |
| Int / Frac | `3A9A:138B`, `3A9A:13DC` | static + via Sin | CONFIRMED |
| Sqrt iteration and stopping rule | `3A9A:13F0..144E` | same | CONFIRMED |
| Sin/Cos reduction, polynomial, constants | `3A9A:144F..14C2`, table `3A9A:14C3` | same | CONFIRMED |
| ArcTan reduction, table, polynomial | `3A9A:1639..1719`, tables `3A9A:171A`, `3A9A:174A` | same | CONFIRMED |
| Polynomial helpers | `3A9A:1768`, `3A9A:176E`, `3A9A:1787` | same | CONFIRMED |
| Mul/div correctly rounded; add deviates (23 of 20,000 random sums) | — | model vs exact rational arithmetic | CONFIRMED (for the sampled inputs) |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-4 (Archaeologist + Spec author) | Initial: all ten game-visible operations, constants, 1,705 vectors |
