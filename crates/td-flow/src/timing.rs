//! Fixed-timestep driver (ADR-0004, MDG §8.4).
//!
//! The simulation advances in whole ticks at the original game's rate; real time is accumulated
//! and converted into a number of ticks to run. Rendering is decoupled and never interpolates in
//! the Classic profile.

use std::time::Duration;

/// Placeholder tick rate until the original's timing is established.
///
/// ASSUMPTION(U-001): 70 Hz, the VGA mode 13h refresh rate, is the most likely candidate.
/// Replace with the value from `docs/spec/core/timing.md` once confirmed.
pub const DEFAULT_TICK_HZ: f64 = 70.0;

/// Longest real-time step accepted in one call; longer pauses (debugger, window drag) are
/// clamped so the simulation does not try to catch up.
pub const MAX_STEP: Duration = Duration::from_millis(250);

/// Converts elapsed real time into whole simulation ticks.
#[derive(Debug, Clone)]
pub struct FixedStep {
    tick_nanos: u128,
    accum_nanos: u128,
    total_ticks: u64,
}

impl FixedStep {
    /// A driver ticking `hz` times per second.
    ///
    /// # Panics
    /// If `hz` is not a positive, finite number.
    pub fn new(hz: f64) -> Self {
        assert!(hz.is_finite() && hz > 0.0, "tick rate must be positive");
        Self {
            tick_nanos: (1e9 / hz).round() as u128,
            accum_nanos: 0,
            total_ticks: 0,
        }
    }

    /// Adds `elapsed` real time and returns how many ticks to run now.
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        self.accum_nanos += elapsed.min(MAX_STEP).as_nanos();
        let n = self.accum_nanos / self.tick_nanos;
        self.accum_nanos -= n * self.tick_nanos;
        self.total_ticks += n as u64;
        n as u32
    }

    /// Ticks run since creation.
    pub fn total_ticks(&self) -> u64 {
        self.total_ticks
    }

    /// Length of one tick.
    pub fn tick(&self) -> Duration {
        Duration::from_nanos(self.tick_nanos as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_drift_over_many_frames() {
        let mut f = FixedStep::new(70.0);
        let frame = Duration::from_nanos(16_666_667); // 60 Hz display
        let ticks: u64 = (0..60 * 600).map(|_| u64::from(f.advance(frame))).sum();
        // 600 s at 70 Hz = 42,000 ticks (within one tick of rounding).
        assert!((41_999..=42_001).contains(&ticks), "{ticks}");
        assert_eq!(f.total_ticks(), ticks);
    }

    #[test]
    fn long_pauses_are_clamped() {
        let mut f = FixedStep::new(70.0);
        assert_eq!(f.advance(Duration::from_secs(10)), 17, "250 ms at 70 Hz");
    }

    #[test]
    fn faster_displays_run_some_frames_without_ticks() {
        let mut f = FixedStep::new(70.0);
        let frame = Duration::from_nanos(6_944_444); // 144 Hz
        let counts: Vec<u32> = (0..10).map(|_| f.advance(frame)).collect();
        assert!(counts.contains(&0) && counts.iter().all(|&c| c <= 1));
    }
}
