//! A YM3812 (OPL2) FM synthesiser, wrapping the ymfm emulator through the `ymfm-sys` crate
//! (BSD-3-Clause; ADR-0005).
//!
//! The interface is deliberately small: reset, register writes, and sample generation at the
//! chip's native rate (clock / 72, about 49,716 Hz for the standard 3.579545 MHz clock).

use std::fmt;
use std::pin::Pin;

use ymfm_sys::ChipPtr;
use ymfm_sys::ffi::{self, ChipType};

/// Standard OPL2 master clock on Sound Blaster and AdLib cards, in Hz.
pub const OPL2_CLOCK: u32 = 3_579_545;

/// An emulated YM3812.
pub struct Opl2 {
    chip: ChipPtr,
}

impl fmt::Debug for Opl2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Opl2")
            .field("sample_rate", &self.sample_rate())
            .finish()
    }
}

impl Opl2 {
    /// Creates a chip running at [`OPL2_CLOCK`].
    pub fn new() -> Self {
        Self::with_clock(OPL2_CLOCK)
    }

    /// Creates a chip running at `clock` Hz.
    pub fn with_clock(clock: u32) -> Self {
        let mut chip = ffi::create_chip(ChipType::Ym3812, clock);
        chip.pin_mut().reset();
        Self { chip }
    }

    /// Native output sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.chip.sample_rate()
    }

    /// Resets every register to its power-on state.
    pub fn reset(&mut self) {
        self.pin().reset();
    }

    /// Writes `value` to OPL register `reg` (address port, then data port).
    pub fn write_reg(&mut self, reg: u8, value: u8) {
        let mut chip = self.pin();
        chip.as_mut().write(0, reg);
        chip.write(1, value);
    }

    /// Fills `out` with mono samples at [`Self::sample_rate`]. The OPL2 is a mono chip; values
    /// are signed 16-bit range stored in `i32`.
    pub fn generate(&mut self, out: &mut [i32]) {
        let channels = self.chip.channels().max(1) as usize;
        if channels == 1 {
            self.pin().generate(out);
            return;
        }
        let mut buf = vec![0i32; out.len() * channels];
        self.pin().generate(&mut buf);
        for (o, frame) in out.iter_mut().zip(buf.chunks_exact(channels)) {
            *o = frame[0];
        }
    }

    fn pin(&mut self) -> Pin<&mut ffi::Chip> {
        self.chip.pin_mut()
    }
}

impl Default for Opl2 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_rate_is_clock_over_72() {
        assert_eq!(Opl2::new().sample_rate(), OPL2_CLOCK / 72);
    }

    #[test]
    fn silent_after_reset_and_sounds_after_key_on() {
        let mut opl = Opl2::new();
        let mut buf = vec![0i32; 2048];
        opl.generate(&mut buf);
        assert!(buf.iter().all(|&s| s == 0), "silent after reset");

        // A plain sine voice on channel 0: modulator off, carrier at full volume, fast envelope.
        for (reg, val) in [
            (0x20, 0x01),
            (0x23, 0x01),
            (0x40, 0x3F),
            (0x43, 0x00),
            (0x60, 0xF0),
            (0x63, 0xF0),
            (0x80, 0x00),
            (0x83, 0x00),
            (0xC0, 0x01),
            (0xA0, 0x41),
            (0xB0, 0x32), // key on, block 4
        ] {
            opl.write_reg(reg, val);
        }
        opl.generate(&mut buf);
        let peak = buf.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak > 1000, "expected an audible tone, peak {peak}");
    }
}
