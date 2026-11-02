//! Deterministic game simulation for Traffic Department 2192.
//!
//! The world advances only through a fixed-rate tick driven by recorded inputs; the state is a
//! plain, hashable data structure. No host floating point in Classic gameplay paths (the
//! original's own `Real` arithmetic is reproduced bit-exactly by [`real48`]), no wall clock, no
//! unseeded randomness, slot-ordered iteration (MDG §8.6). Implemented from `docs/spec/sim/` and
//! `docs/spec/core/`.

pub mod real48;
pub mod rng;

pub use real48::Real48;
pub use rng::BpRandom;
