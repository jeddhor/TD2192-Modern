//! Deterministic game simulation for Traffic Department 2192.
//!
//! The world advances only through a fixed-rate tick driven by recorded inputs; the state is a
//! plain, hashable data structure. No floating point in Classic gameplay paths, no wall clock,
//! no unseeded randomness, slot-ordered iteration (MDG §8.6). Implemented from `docs/spec/sim/`.
