//! Creative (Sound Blaster) ADPCM decoding.
//!
//! Spec: `docs/spec/audio/voc-and-adpcm.md` §3 (rev 1). Fidelity: T0 (pending A/B, M3-08).

/// Code width of an ADPCM scheme in bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// 4-bit codes, two per byte.
    Four,
    /// 3-bit codes, three per byte (the "2.6-bit" scheme).
    Three,
    /// 2-bit codes, four per byte.
    Two,
}

impl Width {
    const fn bits(self) -> u8 {
        match self {
            Width::Four => 4,
            Width::Three => 3,
            Width::Two => 2,
        }
    }
}

/// Decoder state carried across blocks (a type-2 continuation keeps it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdpcmState {
    /// Current sample, centred: unsigned PCM = `sample + 128`.
    sample: i16,
    /// Step exponent 0..=3.
    step: u8,
}

impl AdpcmState {
    /// Starts from an unsigned 8-bit reference sample.
    pub fn from_reference(reference: u8) -> Self {
        Self {
            sample: i16::from(reference) - 128,
            step: 0,
        }
    }

    /// Decodes one code of `width`, returning the unsigned 8-bit output sample.
    pub fn decode(&mut self, code: u8, width: Width) -> u8 {
        let w = width.bits();
        let shift = 4 - w;
        let sign = code & (1 << (w - 1)) != 0;
        let mag = code & ((1 << (w - 1)) - 1);
        let delta = i16::from(mag) << (self.step + shift);
        self.sample = if sign {
            self.sample - delta
        } else {
            self.sample + delta
        }
        .clamp(-128, 127);
        if mag >= 2 * w - 3 && self.step < 3 {
            self.step += 1;
        } else if mag == 0 && self.step > 0 {
            self.step -= 1;
        }
        (self.sample + 128) as u8
    }
}

/// Splits a byte into its codes, most significant first.
fn codes(byte: u8, width: Width) -> impl Iterator<Item = u8> {
    let parts: [u8; 4] = match width {
        Width::Four => [byte >> 4, byte & 0x0F, 0, 0],
        Width::Three => [byte >> 5, (byte >> 2) & 7, (byte & 3) << 1, 0],
        Width::Two => [byte >> 6, (byte >> 4) & 3, (byte >> 2) & 3, byte & 3],
    };
    let n = match width {
        Width::Four => 2,
        Width::Three => 3,
        Width::Two => 4,
    };
    parts.into_iter().take(n)
}

/// Decodes a block whose first byte is a reference sample; appends unsigned 8-bit PCM to `out`
/// and returns the state for a following continuation block.
pub fn decode_block_with_reference(data: &[u8], width: Width, out: &mut Vec<u8>) -> AdpcmState {
    let Some((&reference, rest)) = data.split_first() else {
        return AdpcmState::default();
    };
    out.push(reference);
    let mut state = AdpcmState::from_reference(reference);
    decode_continuation(rest, width, &mut state, out);
    state
}

/// Decodes data that continues from `state` (no reference byte).
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

    #[test]
    fn spec_test_vectors() {
        assert_eq!(decode(&[0x80, 0x70], Width::Four), [0x80, 0x87, 0x87]);
        assert_eq!(decode(&[0x80, 0xF8], Width::Four), [0x80, 0x79, 0x79]);
        assert_eq!(
            decode(&[0x80, 0x55], Width::Two),
            [0x80, 0x84, 0x8C, 0x9C, 0xBC]
        );
        assert_eq!(
            *decode(&[0xFE, 0x77, 0x77], Width::Four).last().unwrap(),
            0xFF
        );
        assert_eq!(
            *decode(&[0x01, 0xFF, 0xFF], Width::Four).last().unwrap(),
            0x00
        );
    }

    #[test]
    fn sample_counts_follow_code_width() {
        let data = [0x80u8; 11];
        assert_eq!(decode(&data, Width::Four).len(), 1 + 2 * 10);
        assert_eq!(decode(&data, Width::Three).len(), 1 + 3 * 10);
        assert_eq!(decode(&data, Width::Two).len(), 1 + 4 * 10);
        assert!(decode(&[], Width::Four).is_empty());
    }

    #[test]
    fn three_bit_last_code_is_shifted() {
        // byte 0b001_001_01: codes 1, 1, 0b10 (=2)
        let mut s = AdpcmState::from_reference(0x80);
        let mut out = Vec::new();
        decode_continuation(&[0b0010_0101], Width::Three, &mut s, &mut out);
        // shift 1: +1<<1 = 2 -> 0x82; +1<<1 = 2 -> 0x84; mag 2 (>=3? no) +2<<1 = 4 -> 0x88
        assert_eq!(out, [0x82, 0x84, 0x88]);
    }
}
