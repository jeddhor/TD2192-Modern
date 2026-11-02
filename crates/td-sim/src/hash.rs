//! Stable state hashing for replay verification (MDG §8.6).
//!
//! FNV-1a (64-bit) over an explicit, field-by-field byte encoding. Unlike `std::hash`, the
//! result is identical on every platform and Rust version, so hashes can be stored in replay
//! files.

use crate::real48::Real48;
use crate::rng::BpRandom;

/// A 64-bit FNV-1a hasher.
#[derive(Debug, Clone)]
pub struct StateHasher(u64);

impl Default for StateHasher {
    fn default() -> Self {
        Self(0xCBF2_9CE4_8422_2325)
    }
}

impl StateHasher {
    /// Feeds bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }

    /// The hash so far.
    pub fn finish(&self) -> u64 {
        self.0
    }
}

/// Types that contribute to the simulation state hash.
pub trait HashState {
    fn hash_state(&self, h: &mut StateHasher);
}

macro_rules! hash_le {
    ($($t:ty),*) => {$(
        impl HashState for $t {
            fn hash_state(&self, h: &mut StateHasher) {
                h.bytes(&self.to_le_bytes());
            }
        }
    )*};
}
hash_le!(u8, i8, u16, i16, u32, i32, u64, i64);

impl HashState for bool {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(&[u8::from(*self)]);
    }
}

impl HashState for Real48 {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(&self.to_bytes());
    }
}

impl HashState for BpRandom {
    fn hash_state(&self, h: &mut StateHasher) {
        self.seed().hash_state(h);
    }
}

impl<T: HashState> HashState for [T] {
    fn hash_state(&self, h: &mut StateHasher) {
        (self.len() as u32).hash_state(h);
        for x in self {
            x.hash_state(h);
        }
    }
}

impl<T: HashState> HashState for Vec<T> {
    fn hash_state(&self, h: &mut StateHasher) {
        self.as_slice().hash_state(h);
    }
}

impl<T: HashState> HashState for Option<T> {
    fn hash_state(&self, h: &mut StateHasher) {
        match self {
            None => h.bytes(&[0]),
            Some(x) => {
                h.bytes(&[1]);
                x.hash_state(h);
            }
        }
    }
}

/// Hashes a value from scratch.
pub fn state_hash<T: HashState + ?Sized>(value: &T) -> u64 {
    let mut h = StateHasher::default();
    value.hash_state(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_reference_values() {
        // Standard FNV-1a 64 test vectors.
        let mut h = StateHasher::default();
        assert_eq!(h.finish(), 0xCBF2_9CE4_8422_2325);
        h.bytes(b"a");
        assert_eq!(h.finish(), 0xAF63_DC4C_8601_EC8C);
    }

    #[test]
    fn structure_changes_the_hash() {
        assert_ne!(state_hash(&vec![1u8, 2]), state_hash(&vec![1u8, 2, 0]));
        assert_ne!(state_hash(&Some(0u8)), state_hash(&None::<u8>));
        assert_eq!(state_hash(&5i16), state_hash(&5i16));
    }
}
