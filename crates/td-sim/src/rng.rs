//! The Borland Pascal 7 random-number generator.
//!
//! Spec: `docs/spec/core/bp7-runtime-semantics.md` §4.1 (rev 1, RNG CONFIRMED). Fidelity: T1.
//! One 32-bit state shared by every caller; the original never calls `Randomize`, so every run
//! of an episode program starts from seed 0. The port must make exactly the same calls in the
//! same order and never add calls of its own to this stream.

use crate::real48::Real48;

/// LCG multiplier (spec §3.4).
pub const MULTIPLIER: u32 = 0x0808_8405;

/// The shared generator (`RandSeed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BpRandom {
    seed: u32,
}

impl BpRandom {
    /// A generator in the state the original starts in (seed 0).
    pub const fn new() -> Self {
        Self { seed: 0 }
    }

    /// A generator with an explicit seed (Enhanced profiles and tests only).
    pub const fn with_seed(seed: u32) -> Self {
        Self { seed }
    }

    /// Current `RandSeed`.
    pub const fn seed(&self) -> u32 {
        self.seed
    }

    /// Advances the state and returns the new seed.
    pub fn step(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(1);
        self.seed
    }

    /// `Random(range)`: `floor(new_seed * range / 2^32)`; the seed advances even for 0.
    pub fn random(&mut self, range: u16) -> u16 {
        let s = u64::from(self.step());
        ((s * u64::from(range)) >> 32) as u16
    }

    /// `Random` without argument: `new_seed / 2^32` as an exact Real48 in [0, 1).
    pub fn random_real(&mut self) -> Real48 {
        let s = self.step();
        Real48::from_u32_scaled(s, -32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_vector_1_seed_sequence() {
        let mut r = BpRandom::new();
        let seq: Vec<u32> = (0..10).map(|_| r.step()).collect();
        assert_eq!(
            seq,
            [
                0x0000_0001,
                0x0808_8406,
                0xDC6D_AC1F,
                0x33DC_589C,
                0x45DE_2B0D,
                0xABF1_8B42,
                0x5195_C04B,
                0x296B_6D78,
                0x5F4B_0359,
                0x6CF8_F4BE
            ]
        );
    }

    fn draws(range: u16) -> Vec<u16> {
        let mut r = BpRandom::new();
        (0..10).map(|_| r.random(range)).collect()
    }

    #[test]
    fn spec_vectors_2_to_5() {
        assert_eq!(draws(100), [0, 3, 86, 20, 27, 67, 31, 16, 37, 42]);
        assert_eq!(draws(320), [0, 10, 275, 64, 87, 214, 101, 51, 119, 136]);
        assert_eq!(draws(2), [0, 0, 1, 0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(
            draws(65535),
            [
                0, 2056, 56428, 13276, 17885, 44016, 20885, 10603, 24394, 27896
            ]
        );
    }

    #[test]
    fn spec_vectors_6_and_7() {
        let mut r = BpRandom::new();
        let out: Vec<u16> = [10, 100, 320, 2, 65535, 7].map(|n| r.random(n)).to_vec();
        assert_eq!(out, [0, 3, 275, 0, 17885, 4]);
        assert_eq!(r.seed(), 0xABF1_8B42);
        let mut r = BpRandom::new();
        assert!((0..10).all(|_| r.random(0) == 0));
        assert_eq!(r.seed(), 0x6CF8_F4BE);
    }

    #[test]
    fn real_random_is_exact() {
        let mut r = BpRandom::new();
        let x = r.random_real(); // seed 1 -> 2^-32
        assert_eq!(x.to_f64(), 2f64.powi(-32));
        let mut r = BpRandom::with_seed(0xFFFF_FFFF);
        let x = r.random_real(); // seed -> 0x08088404... always < 1
        assert!(x.to_f64() < 1.0);
    }
}
