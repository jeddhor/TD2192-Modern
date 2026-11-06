//! Creative (Sound Blaster) ADPCM decoding: the DSP's algorithm.
//!
//! Spec: `docs/spec/audio/voc-and-adpcm.md` §3 (rev 3). Fidelity: T0 for the decoded 8-bit
//! stream (synthetic vectors §4.1, file hashes §4.2, oracle correlation §4.3).
//!
//! WORX copies the sample bytes to the Sound Blaster unchanged and the DSP decodes them, so the
//! reference decoder is the DSP's (BUG-4, U-400): an unsigned 8-bit value that starts at the
//! reference byte (which is not output) and a step that starts at 0; delta = m at step 0 and
//! (2m + 1) · 2^(step − 1) above it (the 2.6-bit codec's step 4: (2m + 1) · 5); the value
//! clamps to 0..255; the step falls on a magnitude-0 code and rises on the largest magnitudes.

/// Code width of an ADPCM scheme in bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// 4-bit codes, two per byte (codec 1).
    Four,
    /// 3-bit codes, three per byte (the "2.6-bit" scheme, codec 2).
    Three,
    /// 2-bit codes, four per byte (codec 3).
    Two,
}

impl Width {
    /// Bits per code.
    const fn bits(self) -> u8 {
        match self {
            Width::Four => 4,
            Width::Three => 3,
            Width::Two => 2,
        }
    }

    /// Codes per data byte (§3 step 2).
    #[must_use]
    pub const fn codes_per_byte(self) -> u8 {
        match self {
            Width::Four => 2,
            Width::Three => 3,
            Width::Two => 4,
        }
    }

    /// Smallest magnitude that raises the step (§3 step 3, UP).
    const fn up(self) -> u8 {
        match self {
            Width::Four => 5,
            Width::Three => 3,
            Width::Two => 1,
        }
    }

    /// Highest step (§3 step 3, TOP).
    const fn top(self) -> u8 {
        match self {
            Width::Four => 3,
            Width::Three => 4,
            Width::Two => 5,
        }
    }
}

/// Decoder state carried across blocks (a type-2 continuation keeps it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdpcmState {
    /// Current unsigned 8-bit sample.
    value: u8,
    /// Step 0..=TOP.
    step: u8,
}

impl AdpcmState {
    /// Starts from the reference byte of a type-1 block: value := the byte, step := 0 (§3
    /// step 1).
    #[must_use]
    pub fn from_reference(reference: u8) -> Self {
        Self {
            value: reference,
            step: 0,
        }
    }

    /// Decodes one code of `width`, returning the unsigned 8-bit output sample (§3 step 3).
    pub fn decode(&mut self, code: u8, width: Width) -> u8 {
        let w = width.bits();
        let sign = code & (1 << (w - 1)) != 0;
        let m = code & ((1 << (w - 1)) - 1);
        let delta: i16 = match (self.step, width) {
            (0, _) => i16::from(m),
            (4, Width::Three) => (2 * i16::from(m) + 1) * 5,
            (s, _) => (2 * i16::from(m) + 1) << (s - 1),
        };
        let v = i16::from(self.value);
        // The clamp keeps the value in 0..=255, so the cast is exact.
        self.value = if sign { v - delta } else { v + delta }.clamp(0, 255) as u8;
        if m == 0 {
            self.step = self.step.saturating_sub(1);
        } else if m >= width.up() && self.step < width.top() {
            self.step += 1;
        }
        self.value
    }
}

/// Splits a byte into its codes, most significant first (§3 step 2). The third 2.6-bit code is
/// the last two bits shifted up: sign = bit 1, magnitude 0 or 2.
fn codes(byte: u8, width: Width) -> impl Iterator<Item = u8> {
    let parts: [u8; 4] = match width {
        Width::Four => [byte >> 4, byte & 0x0F, 0, 0],
        Width::Three => [byte >> 5, (byte >> 2) & 7, (byte & 3) << 1, 0],
        Width::Two => [byte >> 6, (byte >> 4) & 3, (byte >> 2) & 3, byte & 3],
    };
    parts.into_iter().take(usize::from(width.codes_per_byte()))
}

/// Decodes a type-1 block whose first byte is the reference: appends the unsigned 8-bit
/// samples to `out` (codes_per_byte × (n − 1) of them; the reference byte is not output) and
/// returns the state for a following continuation block.
pub fn decode_block_with_reference(data: &[u8], width: Width, out: &mut Vec<u8>) -> AdpcmState {
    let Some((&reference, rest)) = data.split_first() else {
        return AdpcmState::default();
    };
    let mut state = AdpcmState::from_reference(reference);
    decode_continuation(rest, width, &mut state, out);
    state
}

/// Decodes data that continues from `state` (a type-2 block: no reference byte).
pub fn decode_continuation(data: &[u8], width: Width, state: &mut AdpcmState, out: &mut Vec<u8>) {
    for &b in data {
        for code in codes(b, width) {
            out.push(state.decode(code, width));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(data: &[u8], width: Width) -> Vec<u8> {
        let mut out = Vec::new();
        decode_block_with_reference(data, width, &mut out);
        out
    }

    /// voc-and-adpcm rev 3 §4.1: the 12 synthetic vectors.
    #[test]
    fn spec_synthetic_vectors() {
        use Width::*;
        let vectors: [(Width, &[u8], &[u8]); 12] = [
            (Four, &[0x80, 0x70], &[0x87, 0x88]),
            (Four, &[0x80, 0xF8], &[0x79, 0x78]),
            (Four, &[0x80, 0x77, 0x77], &[0x87, 0x96, 0xB4, 0xF0]),
            (Four, &[0xFE, 0x77], &[0xFF, 0xFF]),
            (
                Four,
                &[0x80, 0x77, 0x70, 0x00],
                &[0x87, 0x96, 0xB4, 0xB8, 0xBA, 0xBB],
            ),
            (Four, &[0x03, 0xFF], &[0x00, 0x00]),
            (
                Two,
                &[0x80, 0x55, 0x55],
                &[0x81, 0x84, 0x8A, 0x96, 0xAE, 0xDE, 0xFF, 0xFF],
            ),
            (
                Two,
                &[0x80, 0x55, 0x00],
                &[0x81, 0x84, 0x8A, 0x96, 0x9E, 0xA2, 0xA4, 0xA5],
            ),
            (Two, &[0x80, 0xFF], &[0x7F, 0x7C, 0x76, 0x6A]),
            (
                Three,
                &[0x80, 0x6D, 0x6D, 0x60],
                &[0x83, 0x8A, 0x94, 0xA2, 0xBE, 0xD7, 0xFA, 0xFF, 0xFF],
            ),
            (Three, &[0x80, 0xFF], &[0x7D, 0x76, 0x6C]),
            (Three, &[0x80, 0x03], &[0x80, 0x80, 0x7E]),
        ];
        for (i, (width, data, expected)) in vectors.iter().enumerate() {
            assert_eq!(decode(data, *width), *expected, "vector {}", i + 1);
        }
    }

    /// The delta tables of §3 (positive codes).
    #[test]
    fn delta_tables() {
        let deltas = |width: Width, step: u8| -> Vec<i16> {
            let n = 1u8 << (width.bits() - 1);
            (0..n)
                .map(|m| {
                    let mut s = AdpcmState { value: 0, step };
                    i16::from(s.decode(m, width))
                })
                .collect()
        };
        assert_eq!(deltas(Width::Four, 3), [4, 12, 20, 28, 36, 44, 52, 60]);
        assert_eq!(deltas(Width::Three, 3), [4, 12, 20, 28]);
        assert_eq!(deltas(Width::Three, 4), [5, 15, 25, 35]);
        assert_eq!(deltas(Width::Two, 5), [16, 48]);
        assert_eq!(deltas(Width::Two, 0), [0, 1]);
    }

    #[test]
    fn sample_counts_follow_code_width() {
        let data = [0x80u8; 11];
        assert_eq!(decode(&data, Width::Four).len(), 2 * 10);
        assert_eq!(decode(&data, Width::Three).len(), 3 * 10);
        assert_eq!(decode(&data, Width::Two).len(), 4 * 10);
        assert!(decode(&[], Width::Four).is_empty());
        assert!(decode(&[0x80], Width::Two).is_empty());
    }

    /// `80h` fill bytes after the data decode to deltas of 0 once the step has fallen to 0, so
    /// the last value is held (§2.1).
    #[test]
    fn fill_bytes_hold_the_last_value() {
        let mut out = Vec::new();
        let mut s = decode_block_with_reference(&[0x80, 0x77, 0x77], Width::Four, &mut out);
        out.clear();
        decode_continuation(&[0x80; 4], Width::Four, &mut s, &mut out);
        assert_eq!(*out.last().unwrap(), out[out.len() - 2]);
    }

    #[test]
    fn continuation_keeps_the_state() {
        let mut whole = Vec::new();
        decode_block_with_reference(&[0x80, 0x77, 0x70, 0x00], Width::Four, &mut whole);
        let mut parts = Vec::new();
        let mut s = decode_block_with_reference(&[0x80, 0x77], Width::Four, &mut parts);
        decode_continuation(&[0x70, 0x00], Width::Four, &mut s, &mut parts);
        assert_eq!(whole, parts);
    }
}
