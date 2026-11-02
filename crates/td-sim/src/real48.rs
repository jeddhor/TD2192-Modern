//! Borland Pascal's 6-byte software floating point (`Real`).
//!
//! Spec: `docs/spec/core/bp7-runtime-semantics.md` §3.3 and §4.2 (rev 1). Fidelity: T1.
//! Implemented so far: the exact parts of the spec (format, LongInt to Real, comparison,
//! `Round`, `Trunc`, exact scaling of 32-bit values). Add, multiply, divide, `Sqrt`, `Sin`,
//! `Cos` and `ArcTan` must be bit-exact and wait for spec follow-up M1-01.b; `to_f64` exists for
//! tests and diagnostics only and must never feed Classic game state.

use std::cmp::Ordering;

/// A Borland `Real` value: exponent byte (bias 129, 0 = zero), 39-bit fraction, sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Real48 {
    /// Byte 0: biased exponent; 0 means the value is 0.
    exp: u8,
    /// Bits 38..0 of the fraction (the leading 1 is implicit).
    frac: u64,
    negative: bool,
}

const FRAC_BITS: u32 = 39;
const FRAC_MASK: u64 = (1 << FRAC_BITS) - 1;
const BIAS: i32 = 129;

/// Errors a Real48 operation raises where the original stops with a runtime error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealError {
    /// Runtime error 207: invalid floating-point operation (e.g. `Round` out of LongInt range).
    InvalidOperation,
}

impl Real48 {
    /// Zero.
    pub const ZERO: Real48 = Real48 {
        exp: 0,
        frac: 0,
        negative: false,
    };

    /// Decodes the six bytes as stored in memory.
    pub fn from_bytes(b: [u8; 6]) -> Self {
        if b[0] == 0 {
            return Self::ZERO;
        }
        let frac = u64::from(b[1])
            | u64::from(b[2]) << 8
            | u64::from(b[3]) << 16
            | u64::from(b[4]) << 24
            | u64::from(b[5] & 0x7F) << 32;
        Self {
            exp: b[0],
            frac,
            negative: b[5] & 0x80 != 0,
        }
    }

    /// Encodes as the six bytes stored in memory (zero is all-zero bytes).
    pub fn to_bytes(self) -> [u8; 6] {
        if self.exp == 0 {
            return [0; 6];
        }
        let f = self.frac;
        [
            self.exp,
            f as u8,
            (f >> 8) as u8,
            (f >> 16) as u8,
            (f >> 24) as u8,
            ((f >> 32) as u8 & 0x7F) | if self.negative { 0x80 } else { 0 },
        ]
    }

    /// True if the value is zero.
    pub fn is_zero(self) -> bool {
        self.exp == 0
    }

    /// Exact conversion from a LongInt (spec §4.2, `3A9A:133D`).
    pub fn from_i32(v: i32) -> Self {
        Self::from_magnitude(u64::from(v.unsigned_abs()), 0, v < 0)
    }

    /// Exact `value * 2^scale` for a 32-bit unsigned value (used by the real `Random`).
    pub fn from_u32_scaled(v: u32, scale: i32) -> Self {
        Self::from_magnitude(u64::from(v), scale, false)
    }

    /// Normalises `mag * 2^scale` (mag < 2^40, so no rounding is needed).
    fn from_magnitude(mag: u64, scale: i32, negative: bool) -> Self {
        if mag == 0 {
            return Self::ZERO;
        }
        let top = 63 - mag.leading_zeros() as i32; // index of the leading 1
        let frac = (mag << (FRAC_BITS as i32 - top)) & FRAC_MASK;
        Self {
            exp: (BIAS + top + scale) as u8,
            frac,
            negative,
        }
    }

    /// Rounds half away from zero to a LongInt (spec §4.2 `Round`, `3A9A:1349`).
    pub fn round(self) -> Result<i32, RealError> {
        self.to_integer(true)
    }

    /// Truncates toward zero to a LongInt (`Trunc`).
    pub fn trunc(self) -> Result<i32, RealError> {
        self.to_integer(false)
    }

    fn to_integer(self, round: bool) -> Result<i32, RealError> {
        if self.exp == 0 {
            return Ok(0);
        }
        let e = i32::from(self.exp) - BIAS; // value = 1.frac * 2^e
        let mant = (1u64 << FRAC_BITS) | self.frac; // 40 bits
        let shift = FRAC_BITS as i32 - e;
        let mag: u64 = if shift <= 0 {
            if -shift > 24 {
                return Err(RealError::InvalidOperation);
            }
            mant << -shift
        } else if shift > 40 {
            0
        } else {
            let whole = mant >> shift;
            let first_dropped = (mant >> (shift - 1)) & 1;
            whole + if round { first_dropped } else { 0 }
        };
        if self.negative {
            if mag > 1 << 31 {
                return Err(RealError::InvalidOperation);
            }
            Ok((mag as i64).wrapping_neg() as i32)
        } else {
            i32::try_from(mag).map_err(|_| RealError::InvalidOperation)
        }
    }

    /// The value as `f64` (exact; tests and diagnostics only).
    pub fn to_f64(self) -> f64 {
        if self.exp == 0 {
            return 0.0;
        }
        let m = 1.0 + self.frac as f64 / (1u64 << FRAC_BITS) as f64;
        let v = m * 2f64.powi(i32::from(self.exp) - BIAS);
        if self.negative { -v } else { v }
    }
}

impl PartialOrd for Real48 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Real48 {
    /// Spec §4.2 compare: zeros are equal; differing signs order by sign; otherwise exponent,
    /// then fraction, inverted for negatives.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.is_zero(), other.is_zero()) {
            (true, true) => return Ordering::Equal,
            (true, false) => {
                return if other.negative {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }
            (false, true) => {
                return if self.negative {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }
            _ => {}
        }
        if self.negative != other.negative {
            return if self.negative {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let mag = (self.exp, self.frac).cmp(&(other.exp, other.frac));
        if self.negative { mag.reverse() } else { mag }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_format_examples() {
        assert_eq!(Real48::from_i32(1).to_bytes(), [0x81, 0, 0, 0, 0, 0]);
        assert_eq!(
            Real48::from_u32_scaled(1, -1).to_bytes(),
            [0x80, 0, 0, 0, 0, 0]
        );
        let half_pi = Real48::from_bytes([0x81, 0x21, 0xA2, 0xDA, 0x0F, 0x49]);
        assert!((half_pi.to_f64() - std::f64::consts::FRAC_PI_2).abs() < 1e-11);
        let neg = Real48::from_bytes([0x81, 0x21, 0xA2, 0xDA, 0x0F, 0xC9]);
        assert!(neg.to_f64() < 0.0);
        assert_eq!(Real48::from_bytes(neg.to_bytes()), neg);
        assert_eq!(Real48::from_bytes([0, 1, 2, 3, 4, 5]), Real48::ZERO);
    }

    #[test]
    fn longint_conversion_is_exact() {
        for v in [0, 1, -1, 3, 100, -320, 65_535, i32::MAX, i32::MIN] {
            let r = Real48::from_i32(v);
            assert_eq!(r.to_f64(), f64::from(v), "{v}");
            assert_eq!(r.trunc(), Ok(v));
            assert_eq!(r.round(), Ok(v));
        }
    }

    #[test]
    fn round_half_away_from_zero() {
        let r = |num: i32, sh: i32| Real48::from_magnitude(num.unsigned_abs().into(), sh, num < 0);
        assert_eq!(r(5, -1).round(), Ok(3), "2.5 -> 3");
        assert_eq!(r(-5, -1).round(), Ok(-3), "-2.5 -> -3");
        assert_eq!(r(5, -1).trunc(), Ok(2));
        assert_eq!(r(-5, -1).trunc(), Ok(-2));
        assert_eq!(r(1, -1).round(), Ok(1), "0.5 -> 1");
        assert_eq!(r(1, -2).round(), Ok(0), "0.25 -> 0");
        assert_eq!(r(1, 31).round(), Err(RealError::InvalidOperation), "2^31");
        assert_eq!(r(-1, 31).round(), Ok(i32::MIN), "-2^31 is allowed");
    }

    #[test]
    fn ordering_matches_values() {
        let vals = [-1000, -3, -1, 0, 1, 2, 3, 1000];
        for a in vals {
            for b in vals {
                assert_eq!(
                    Real48::from_i32(a).cmp(&Real48::from_i32(b)),
                    a.cmp(&b),
                    "{a} vs {b}"
                );
            }
        }
    }
}
