//! Fixed-timestep driver (ADR-0004, MDG §8.4).
//!
//! The simulation advances in whole ticks at the original game's rate; real time is accumulated
//! and converted into a number of ticks to run. Rendering is decoupled and never interpolates in
//! the Classic profile.

use std::time::Duration;

use crate::screen::{FLIGHT_TICK_HZ, RETRACE_HZ};

/// The BIOS tick (and with it DOS time) while a 96 Hz CMF song plays: 1,193,182 / (12,428 × 5)
/// = 19.2015 Hz, the same clock as the flight tick ([`FLIGHT_TICK_HZ`]). Classic uses this rate
/// for every DOS-time wait, whatever the sound configuration. Spec: `docs/spec/core/timing.md`
/// rev 5 §4.2, §4.4 and §5; ADR-0007 (accepted 2026-11-06).
pub const BIOS_TICK_HZ: f64 = FLIGHT_TICK_HZ;

/// BIOS ticks per second of DOS time: DOS time advances 6,553,600 / 1,193,182 centiseconds per
/// BIOS tick, so 100 cs take 1,193,182 / 65,536 = 18.2065 ticks. Spec: timing rev 5 §4.4.
pub const BIOS_TICKS_PER_DOS_SECOND: f64 = 1_193_182.0 / 65_536.0;

/// Retrace frames (70.086 Hz) that `Flow_WaitSecondsOrKey(n)` (`file0001:051A:1AA3`, the E2
/// and E3 copies) waits when no key comes: n seconds of DOS time, which advances only with the
/// BIOS tick, so round(n × 18.2065) BIOS ticks at [`BIOS_TICK_HZ`] (the original takes
/// ceil(n × 18.2065) ticks or one less, depending on where in a BIOS tick the wait starts).
/// 5 s → 332 frames, 10 s → 664 frames (measured 328 and 664, C-0002, C-0170). Spec:
/// timing rev 5 §4.4, `docs/spec/flow/screens.md` rev 3 §2.2 (BUG-7). Fidelity: T2.
///
/// `Crt.Delay(ms)` pauses are real milliseconds and do not use this.
#[must_use]
pub fn dos_seconds_frames(n: u32) -> u32 {
    let ticks = (f64::from(n) * BIOS_TICKS_PER_DOS_SECOND).round();
    (ticks * RETRACE_HZ / BIOS_TICK_HZ).round() as u32
}

/// Retrace frames of a `Crt.Delay(ms)` pause: real milliseconds (Crt calibrates its loop
/// against the standard 18.2 Hz tick before WORX speeds the timer up). Spec: timing rev 4
/// §4.4 item 2. Fidelity: T2. The typed front-end screens use [`typing_delay_frames`].
#[must_use]
pub fn delay_ms_frames(ms: u32) -> u32 {
    (f64::from(ms) * RETRACE_HZ / 1000.0).round() as u32
}

/// Retrace frames per millisecond of a `Crt.Delay` between two typed lines of a front-end
/// screen while music plays: n ms plus the interrupt handlers' time, about 0.6 % more than
/// real time (70.086 Hz × 1.006 ≈ 70.5 Hz). Spec: `docs/spec/render/text.md` rev 5 §4.3 item
/// 5 and its port rule (U-521, BUG-20; C-0270, C-0274, C-0284, C-0289).
pub const TYPING_DELAY_FRAMES_PER_MS: f64 = 0.0705;

/// Frames of a `Crt.Delay(ms)` between two typed lines (text rev 5 §4.3 port rule):
/// round(ms × 0.0705), so the next line starts in frame F + 1 + that (F the frame of the
/// previous line's last character): 500 ms → 35 (F + 36), 2000 ms → 141 (F + 142). T2.
#[must_use]
pub fn typing_delay_frames(ms: u32) -> u32 {
    (f64::from(ms) * TYPING_DELAY_FRAMES_PER_MS).round() as u32
}

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
    fn dos_time_waits_count_bios_ticks() {
        // timing rev 4 §4.4, §6 vectors 7 and 8: 91 and 182 BIOS ticks at 19.2015 Hz.
        assert!((BIOS_TICK_HZ - 19.2015).abs() < 1e-4);
        assert!((BIOS_TICKS_PER_DOS_SECOND - 18.2065).abs() < 1e-4);
        let frames: Vec<u32> = [5, 10, 15, 25, 30, 45]
            .into_iter()
            .map(dos_seconds_frames)
            .collect();
        assert_eq!(frames, [332, 664, 996, 1661, 1993, 2989]);
        // Crt.Delay stays in real milliseconds.
        assert_eq!(delay_ms_frames(500), 35);
        assert_eq!(delay_ms_frames(100), 7);
        assert_eq!(delay_ms_frames(2000), 140);
        // Between typed lines (text rev 5 §4.3, BUG-20): F + 36 and F + 142.
        assert_eq!(typing_delay_frames(500), 35);
        assert_eq!(typing_delay_frames(2000), 141);
        assert_eq!(typing_delay_frames(100), 7);
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
