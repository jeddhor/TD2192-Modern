//! Borland Pascal's 6-byte software floating point (`Real`), bit-exact.
//!
//! Spec: `docs/spec/core/real48.md` (rev 1) and `docs/spec/core/bp7-runtime-semantics.md` §3.3.
//! Fidelity: T1. Every operation follows the spec's integer procedure; the result bytes match
//! the original routines on all 1,705 vectors in `docs/spec/core/real48-vectors.csv`.
//!
//! Values keep all six bytes, because zeros may carry a sign and stray bytes that survive some
//! operations (spec §5). The game-visible entry points return [`RealError`] where the original
//! stops with a runtime error; internal use (inside `Sqrt`, `Sin`, `Cos`, `ArcTan`) ignores
//! overflow as the original does. [`Real48::to_f64`] is for tests and diagnostics only.

use std::cmp::Ordering;

/// A Borland `Real` value, as its six bytes in memory order (exponent byte first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Real48([u8; 6]);

/// Runtime errors the original raises from Real48 operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealError {
    /// RunError 200: division by zero.
    DivisionByZero,
    /// RunError 205: floating-point overflow.
    Overflow,
    /// RunError 207: invalid floating-point operation.
    InvalidOperation,
}

impl RealError {
    /// The original's runtime error number.
    pub const fn code(self) -> u16 {
        match self {
            RealError::DivisionByZero => 200,
            RealError::Overflow => 205,
            RealError::InvalidOperation => 207,
        }
    }
}

const HIDDEN: u64 = 1 << 39;
const FRAC_MASK: u64 = HIDDEN - 1;
const W_TOP: u64 = 1 << 48;
const W_NORM: u64 = 1 << 47;

const fn r(b: [u8; 6]) -> Real48 {
    Real48(b)
}

/// 1.
pub const ONE: Real48 = r([0x81, 0, 0, 0, 0, 0]);
const HALF_PI: Real48 = r([0x81, 0x21, 0xA2, 0xDA, 0x0F, 0x49]);
const NEG_HALF_PI: Real48 = r([0x81, 0x21, 0xA2, 0xDA, 0x0F, 0xC9]);
const PI: Real48 = r([0x82, 0x21, 0xA2, 0xDA, 0x0F, 0x49]);
const TWO_PI: Real48 = r([0x83, 0x21, 0xA2, 0xDA, 0x0F, 0x49]);
const ATAN_SMALL: Real48 = r([0x7E, 0x4A, 0x8E, 0xE9, 0x6F, 0x0C]);

const SIN_C: [Real48; 7] = [
    r([0x58, 0x9D, 0x39, 0x9F, 0x3F, 0xD7]),
    r([0x60, 0x43, 0x9D, 0x30, 0x92, 0x30]),
    r([0x67, 0xAA, 0x3F, 0x28, 0x32, 0xD7]),
    r([0x6E, 0xB6, 0x2A, 0x1D, 0xEF, 0x38]),
    r([0x74, 0x0D, 0xD0, 0x00, 0x0D, 0xD0]),
    r([0x7A, 0x88, 0x88, 0x88, 0x88, 0x08]),
    r([0x7E, 0xAB, 0xAA, 0xAA, 0xAA, 0xAA]),
];

const ATAN_C: [Real48; 5] = [
    r([0x7D, 0xE8, 0xA2, 0x8B, 0x2E, 0xBA]),
    r([0x7D, 0x8E, 0xE3, 0x38, 0x8E, 0x63]),
    r([0x7E, 0x49, 0x92, 0x24, 0x49, 0x92]),
    r([0x7E, 0xCD, 0xCC, 0xCC, 0xCC, 0x4C]),
    r([0x7F, 0xAB, 0xAA, 0xAA, 0xAA, 0xAA]),
];

/// ArcTan reduction rows: (upper bound, tan of centre, centre angle). The last row has no bound.
const ATAN_ROWS: [(Option<Real48>, Real48, Real48); 3] = [
    (
        Some(r([0x7F, 0xE7, 0xCF, 0xCC, 0x13, 0x54])),
        r([0x7F, 0xF6, 0xF4, 0xA2, 0x30, 0x09]),
        r([0x7F, 0x6A, 0xC1, 0x91, 0x0A, 0x06]),
    ),
    (
        Some(r([0x80, 0xB5, 0x9E, 0x8A, 0x6F, 0x44])),
        r([0x80, 0x82, 0x2C, 0x3A, 0xCD, 0x13]),
        r([0x80, 0x6A, 0xC1, 0x91, 0x0A, 0x06]),
    ),
    (
        None,
        r([0x81, 0x00, 0x00, 0x00, 0x00, 0x00]),
        r([0x80, 0x21, 0xA2, 0xDA, 0x0F, 0x49]),
    ),
];

/// Result of a core operation: the value and whether it overflowed.
type Core = (Real48, bool);

impl Real48 {
    /// Canonical zero.
    pub const ZERO: Real48 = Real48([0; 6]);

    /// From the six bytes in memory order.
    pub const fn from_bytes(b: [u8; 6]) -> Self {
        Self(b)
    }

    /// The six bytes in memory order.
    pub const fn to_bytes(self) -> [u8; 6] {
        self.0
    }

    /// Exponent byte (0 = zero).
    pub const fn exponent(self) -> u8 {
        self.0[0]
    }

    /// Sign bit.
    pub const fn sign(self) -> bool {
        self.0[5] & 0x80 != 0
    }

    /// True if the value is zero (exponent byte 0, whatever the other bytes hold).
    pub const fn is_zero(self) -> bool {
        self.0[0] == 0
    }

    /// 40-bit significand including the hidden bit.
    fn mant(self) -> u64 {
        let b = self.0;
        HIDDEN
            | u64::from(b[1])
            | u64::from(b[2]) << 8
            | u64::from(b[3]) << 16
            | u64::from(b[4]) << 24
            | u64::from(b[5] & 0x7F) << 32
    }

    fn pack(sign: bool, e: u8, m: u64) -> Self {
        debug_assert!((HIDDEN..2 * HIDDEN).contains(&m));
        let f = m & FRAC_MASK;
        Self([
            e,
            f as u8,
            (f >> 8) as u8,
            (f >> 16) as u8,
            (f >> 24) as u8,
            ((f >> 32) as u8 & 0x7F) | if sign { 0x80 } else { 0 },
        ])
    }

    fn with_sign(self, sign: bool) -> Self {
        let mut b = self.0;
        b[5] = (b[5] & 0x7F) | if sign { 0x80 } else { 0 };
        Self(b)
    }

    fn with_exponent(self, e: u8) -> Self {
        let mut b = self.0;
        b[0] = e;
        Self(b)
    }

    /// Unary minus: flips the sign bit (also of zero, giving "-0", spec §4.4).
    pub fn negated(self) -> Self {
        self.with_sign(!self.sign())
    }

    /// Absolute value: clears the sign bit.
    pub fn abs(self) -> Self {
        self.with_sign(false)
    }

    // ----- core operations (spec §4.1-4.3), overflow reported, not raised -----

    fn core_add(a: Self, b: Self) -> Core {
        if b.is_zero() {
            return (a, false);
        }
        if a.is_zero() {
            return (b, false);
        }
        let (big, small) = if a.exponent() > b.exponent() {
            (a, b)
        } else {
            (b, a)
        };
        let k = u32::from(big.exponent() - small.exponent());
        if k >= 41 {
            return (big, false);
        }
        let wr = big.mant() << 8;
        let wt = (small.mant() << 8) >> k;
        let mut e = i32::from(big.exponent());
        let (mut w, sign);
        if big.sign() == small.sign() {
            w = wr + wt;
            sign = big.sign();
            if w >= W_TOP {
                w >>= 1;
                e += 1;
                if e == 256 {
                    return (Self::ZERO, true);
                }
            }
        } else {
            let d = wt as i64 - wr as i64;
            if d == 0 {
                return (Self::ZERO, false);
            }
            if d < 0 {
                w = (-d) as u64;
                sign = big.sign();
            } else {
                w = d as u64;
                sign = small.sign();
            }
            while w < W_NORM {
                w <<= 1;
                e -= 1;
                if e == 0 {
                    return (Self::ZERO, false);
                }
            }
        }
        Self::round_pack(sign, e, w)
    }

    /// `round48` then pack (spec §2).
    fn round_pack(sign: bool, mut e: i32, mut w: u64) -> Core {
        w += 128;
        if w >= W_TOP {
            w = W_NORM;
            e += 1;
        }
        if e >= 256 {
            return (Self::ZERO, true);
        }
        (Self::pack(sign, e as u8, w >> 8), false)
    }

    fn core_sub(a: Self, b: Self) -> Core {
        Self::core_add(a, b.negated())
    }

    fn core_mul(a: Self, b: Self) -> Core {
        if a.is_zero() || b.is_zero() {
            return (Self::ZERO, false);
        }
        let prod = u128::from(a.mant()) * u128::from(b.mant());
        let mut p = (prod >> 32) as u64;
        let mut e = i32::from(a.exponent()) + i32::from(b.exponent()) - 128;
        if p < W_NORM {
            p <<= 1;
            e -= 1;
        }
        p += 128;
        if p >= W_TOP {
            p = W_NORM;
            e += 1;
        }
        if e <= 0 {
            return (Self::ZERO, false);
        }
        if e >= 256 {
            return (Self::ZERO, true);
        }
        (Self::pack(a.sign() ^ b.sign(), e as u8, p >> 8), false)
    }

    /// Core divide; the caller guarantees `b` is not zero.
    fn core_div(a: Self, b: Self) -> Core {
        if a.is_zero() {
            return (Self::ZERO, false);
        }
        let q = ((u128::from(a.mant()) << 41) / u128::from(b.mant())) as u64;
        let mut w = (q << 6) + 63;
        let mut e = i32::from(a.exponent()) - i32::from(b.exponent()) + 129;
        if w < W_NORM {
            w = (w << 1) % W_TOP;
            e -= 1;
        }
        w += 128;
        if w >= W_TOP {
            w = W_NORM;
            e += 1;
        }
        if e <= 0 {
            return (Self::ZERO, false);
        }
        if e >= 256 {
            return (Self::ZERO, true);
        }
        (Self::pack(a.sign() ^ b.sign(), e as u8, w >> 8), false)
    }

    fn checked(c: Core) -> Result<Self, RealError> {
        if c.1 {
            Err(RealError::Overflow)
        } else {
            Ok(c.0)
        }
    }

    // ----- game-visible operations -----

    /// `a + b` (RunError 205 on overflow).
    pub fn checked_add(self, b: Self) -> Result<Self, RealError> {
        Self::checked(Self::core_add(self, b))
    }

    /// `a - b` (RunError 205 on overflow).
    pub fn checked_sub(self, b: Self) -> Result<Self, RealError> {
        Self::checked(Self::core_sub(self, b))
    }

    /// `a * b` (RunError 205 on overflow).
    pub fn checked_mul(self, b: Self) -> Result<Self, RealError> {
        Self::checked(Self::core_mul(self, b))
    }

    /// `a / b` (RunError 200 for a zero divisor, checked first; 205 on overflow).
    pub fn checked_div(self, b: Self) -> Result<Self, RealError> {
        if b.is_zero() {
            return Err(RealError::DivisionByZero);
        }
        Self::checked(Self::core_div(self, b))
    }

    /// Comparison (spec §4.4): sign bits first (so -0 < +0), then the byte key.
    pub fn compare(self, b: Self) -> Ordering {
        if self.sign() != b.sign() {
            return if self.sign() {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        if self.is_zero() && b.is_zero() {
            return Ordering::Equal;
        }
        let key = |x: Self| [x.0[0], x.0[5], x.0[4], x.0[3], x.0[2], x.0[1]];
        let order = key(self).cmp(&key(b));
        if self.sign() { order.reverse() } else { order }
    }

    /// Exact LongInt to Real (spec §4.5).
    pub fn from_i32(v: i32) -> Self {
        if v == 0 {
            return Self::ZERO;
        }
        let mag = u64::from(v.unsigned_abs());
        let n = 64 - mag.leading_zeros(); // bit length 1..=32
        Self::pack(v < 0, (128 + n) as u8, mag << (40 - n))
    }

    /// Exact `v * 2^scale` for a 32-bit value (used by the real-valued `Random`).
    pub fn from_u32_scaled(v: u32, scale: i32) -> Self {
        if v == 0 {
            return Self::ZERO;
        }
        let mag = u64::from(v);
        let n = 64 - mag.leading_zeros();
        Self::pack(false, (128 + n as i32 + scale) as u8, mag << (40 - n))
    }

    fn to_long(self, rounding: bool) -> Result<i32, RealError> {
        let e = i32::from(self.exponent());
        if e > 160 {
            return Err(RealError::InvalidOperation);
        }
        if e < 128 {
            return Ok(0);
        }
        let m = self.mant();
        let shift = 168 - e; // 8..=40
        let mut i = m >> shift;
        let first_frac = (m >> (shift - 1)) & 1;
        if rounding && first_frac == 1 {
            i += 1;
            if i >= 1 << 32 {
                return Err(RealError::InvalidOperation);
            }
        }
        if i == 0 {
            return Ok(0);
        }
        let v = if self.sign() {
            (i as u32).wrapping_neg()
        } else {
            i as u32
        };
        if (v >> 31 == 1) != self.sign() {
            return Err(RealError::InvalidOperation);
        }
        Ok(v as i32)
    }

    /// `Round`: half away from zero (RunError 207 out of LongInt range).
    pub fn round(self) -> Result<i32, RealError> {
        self.to_long(true)
    }

    /// `Trunc` (RunError 207 out of LongInt range).
    pub fn trunc(self) -> Result<i32, RealError> {
        self.to_long(false)
    }

    /// Internal `Int` (spec §4.7).
    fn int(self) -> Self {
        let e = self.exponent();
        if e >= 168 {
            return self;
        }
        if e <= 128 {
            return self.with_exponent(0);
        }
        let clear = 168 - u32::from(e);
        let field = (self.mant() & FRAC_MASK) | (u64::from(self.0[5] & 0x80) << 32);
        let kept = field & !((1u64 << clear) - 1);
        let mut b = self.0;
        for (i, byte) in b[1..6].iter_mut().enumerate() {
            *byte = (kept >> (8 * i)) as u8;
        }
        b[5] = (b[5] & 0x7F) | (self.0[5] & 0x80);
        Self(b)
    }

    /// Internal `Frac` = `x - Int(x)`.
    fn frac(self) -> Self {
        Self::core_sub(self, self.int()).0
    }

    /// Odd polynomial (spec §4.8).
    fn poly_odd(x: Self, c: &[Self]) -> Self {
        let y = Self::core_mul(x, x).0;
        let mut acc = Self::core_mul(c[0], y).0;
        for &ci in &c[1..] {
            acc = Self::core_add(acc, ci).0;
            acc = Self::core_mul(acc, y).0;
        }
        acc = Self::core_add(acc, ONE).0;
        Self::core_mul(acc, x).0
    }

    /// `Sqrt` (spec §4.9; RunError 207 for a negative nonzero argument).
    pub fn sqrt(self) -> Result<Self, RealError> {
        if self.is_zero() {
            return Ok(self);
        }
        if self.sign() {
            return Err(RealError::InvalidOperation);
        }
        let e = i32::from(self.exponent());
        let eg = 128 + (e - 128).div_euclid(2);
        let mut g = self.with_exponent(eg as u8);
        let limit = eg - 20;
        loop {
            let q = Self::core_div(self, g).0;
            let n = Self::core_add(q, g).0;
            let n = n.with_exponent(n.exponent().wrapping_sub(1));
            let d = Self::core_sub(n, g).0;
            g = n;
            if i32::from(d.exponent()) < limit {
                return Ok(g);
            }
        }
    }

    /// `Sin` (spec §4.10).
    pub fn sin(self) -> Self {
        let mut x = self;
        if x.exponent() < 108 {
            return x;
        }
        if x.abs().compare(TWO_PI) != Ordering::Less {
            x = Self::core_mul(Self::core_div(x, TWO_PI).0.frac(), TWO_PI).0;
        }
        if x.sign() {
            x = Self::core_add(x, TWO_PI).0;
        }
        let upper = x.compare(PI) != Ordering::Less;
        if upper {
            x = Self::core_sub(x, PI).0;
        }
        if x.compare(HALF_PI) != Ordering::Less {
            x = Self::core_add(x.with_sign(true), PI).0;
        }
        if x.exponent() >= 108 {
            x = Self::poly_odd(x, &SIN_C);
        }
        if upper && !x.is_zero() {
            x = x.negated();
        }
        x
    }

    /// `Cos` (spec §4.10): `sin(pi/2 - x)`.
    pub fn cos(self) -> Self {
        let mut y = Self::core_add(self, NEG_HALF_PI).0;
        if !y.is_zero() {
            y = y.negated();
        }
        y.sin()
    }

    /// `ArcTan` (spec §4.11).
    pub fn arctan(self) -> Self {
        if self.is_zero() {
            return self;
        }
        let negative = self.sign();
        let mut x = self.abs();
        let mut inverted = false;
        if x.compare(ONE) != Ordering::Less {
            x = Self::core_div(ONE, x).0;
            inverted = true;
        }
        let mut res = if x.compare(ATAN_SMALL) == Ordering::Less {
            Self::poly_odd(x, &ATAN_C)
        } else {
            let (_, t, a) = ATAN_ROWS
                .iter()
                .find(|(bound, _, _)| bound.is_none_or(|b| x.compare(b) == Ordering::Less))
                .copied()
                .expect("last row has no bound");
            let num = Self::core_sub(x, t).0;
            let den = Self::core_add(Self::core_mul(x, t).0, ONE).0;
            let u = Self::core_div(num, den).0;
            Self::core_add(Self::poly_odd(u, &ATAN_C), a).0
        };
        if inverted {
            res = Self::core_sub(HALF_PI, res).0;
        }
        if negative {
            res = res.with_sign(true);
        }
        res
    }

    /// The value as `f64` (exact; tests and diagnostics only, never Classic game state).
    pub fn to_f64(self) -> f64 {
        if self.is_zero() {
            return 0.0;
        }
        let v = self.mant() as f64 * 2f64.powi(i32::from(self.exponent()) - 168);
        if self.sign() { -v } else { v }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Real48 {
        let mut b = [0u8; 6];
        for (i, byte) in b.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
        }
        Real48(b)
    }

    #[test]
    fn spec_examples() {
        assert_eq!(Real48::from_i32(1), ONE);
        assert_eq!(
            Real48::from_u32_scaled(1, -1).to_bytes(),
            [0x80, 0, 0, 0, 0, 0]
        );
        assert_eq!(Real48::ZERO.cos(), hex("80FAFFFFFF7F"), "Cos(0) is not 1");
        assert_eq!(PI.sin().exponent(), 0, "Sin(pi) = 0");
        let neg_zero = Real48::ZERO.negated();
        assert_eq!(neg_zero.compare(Real48::ZERO), Ordering::Less, "-0 < +0");
        assert_eq!(
            Real48::from_i32(5).checked_div(Real48::ZERO),
            Err(RealError::DivisionByZero)
        );
    }

    #[test]
    fn rounding_is_half_away_from_zero() {
        let half = |n: i32| {
            Real48::from_i32(n)
                .checked_mul(hex("800000000000"))
                .unwrap()
        };
        assert_eq!(half(5).round(), Ok(3));
        assert_eq!(half(-5).round(), Ok(-3));
        assert_eq!(half(5).trunc(), Ok(2));
        assert_eq!(Real48::from_i32(i32::MIN).round(), Ok(i32::MIN));
    }

    /// Every row of the vector file generated from the original routines (spec §6.1).
    #[test]
    fn matches_all_vectors_from_the_original() {
        let csv = include_str!("../../../docs/spec/core/real48-vectors.csv");
        let mut checked = 0;
        for (line_no, line) in csv.lines().enumerate().skip(1) {
            let f: Vec<&str> = line.split(',').collect();
            let (op, a, b, want) = (f[0], f[1], f[2], f[3]);
            let err = |e: RealError| format!("E{}", e.code());
            let real = |r: Result<Real48, RealError>| match r {
                Ok(v) => v.to_bytes().iter().map(|x| format!("{x:02X}")).collect(),
                Err(e) => err(e),
            };
            let long = |r: Result<i32, RealError>| r.map_or_else(err, |v| v.to_string());
            let got = match op {
                "add" => real(hex(a).checked_add(hex(b))),
                "sub" => real(hex(a).checked_sub(hex(b))),
                "mul" => real(hex(a).checked_mul(hex(b))),
                "div" => real(hex(a).checked_div(hex(b))),
                "cmp" => match hex(a).compare(hex(b)) {
                    Ordering::Less => "-1".into(),
                    Ordering::Equal => "0".into(),
                    Ordering::Greater => "1".into(),
                },
                "from_long" => real(Ok(Real48::from_i32(a.parse().unwrap()))),
                "round" => long(hex(a).round()),
                "trunc" => long(hex(a).trunc()),
                "sqrt" => real(hex(a).sqrt()),
                "sin" => real(Ok(hex(a).sin())),
                "cos" => real(Ok(hex(a).cos())),
                "arctan" => real(Ok(hex(a).arctan())),
                other => panic!("unknown op {other}"),
            };
            assert_eq!(got, want, "line {}: {line}", line_no + 1);
            checked += 1;
        }
        assert_eq!(checked, 1705);
    }
}
