//! VGA palettes: 256 entries of 6-bit red, green and blue.
//!
//! Spec: `docs/re/FORMATS.md` §1 (CONFIRMED). Fidelity: T0.
//! Files: `td.pal`, `tdmen.pal`, and the first 768 bytes of every full-screen image.

use crate::{FormatError, Result};

/// Size of a palette in bytes.
pub const PALETTE_LEN: usize = 768;

/// One VGA DAC colour with 6-bit components (0..=63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb6 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb6 {
    /// Expands to 8 bits per component by bit replication, so that 63 maps to 255.
    pub const fn to_rgb8(self) -> [u8; 3] {
        [expand6(self.r), expand6(self.g), expand6(self.b)]
    }
}

/// Expands a 6-bit DAC component (0..=63) to 8 bits: `(v << 2) | (v >> 4)`.
pub const fn expand6(v: u8) -> u8 {
    let v = v & 0x3F;
    (v << 2) | (v >> 4)
}

/// A 256-colour VGA palette.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Palette {
    pub colors: [Rgb6; 256],
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            colors: [Rgb6::default(); 256],
        }
    }
}

impl Palette {
    /// Parses exactly 768 bytes; every component must be at most 63.
    pub fn parse(data: &[u8]) -> Result<Self> {
        FormatError::check_size("palette", data, PALETTE_LEN)?;
        Self::parse_prefix(data)
    }

    /// Parses the first 768 bytes of `data` (for files that start with a palette).
    pub fn parse_prefix(data: &[u8]) -> Result<Self> {
        let bytes = data
            .get(..PALETTE_LEN)
            .ok_or_else(|| FormatError::Truncated {
                format: "palette",
                offset: data.len(),
                needed: PALETTE_LEN - data.len(),
            })?;
        if let Some(offset) = bytes.iter().position(|&c| c > 63) {
            return Err(FormatError::Invalid {
                format: "palette",
                offset,
                detail: format!("component {} exceeds 63", bytes[offset]),
            });
        }
        let mut colors = [Rgb6::default(); 256];
        for (c, rgb) in colors.iter_mut().zip(bytes.chunks_exact(3)) {
            *c = Rgb6 {
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
            };
        }
        Ok(Self { colors })
    }

    /// The palette as 768 bytes in file order.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.colors.iter().flat_map(|c| [c.r, c.g, c.b]).collect()
    }

    /// The palette expanded to 8-bit RGB triples.
    pub fn to_rgb8(&self) -> [[u8; 3]; 256] {
        let mut out = [[0; 3]; 256];
        for (o, c) in out.iter_mut().zip(self.colors.iter()) {
            *o = c.to_rgb8();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_six_bit_components() {
        assert_eq!(expand6(0), 0);
        assert_eq!(expand6(1), 4);
        assert_eq!(expand6(32), 130);
        assert_eq!(expand6(63), 255);
    }

    #[test]
    fn round_trips_and_validates() {
        let mut bytes = vec![0u8; PALETTE_LEN];
        bytes[3..6].copy_from_slice(&[63, 32, 1]);
        let pal = Palette::parse(&bytes).unwrap();
        assert_eq!(pal.colors[1], Rgb6 { r: 63, g: 32, b: 1 });
        assert_eq!(pal.to_bytes(), bytes);
        assert_eq!(pal.to_rgb8()[1], [255, 130, 4]);

        bytes[10] = 64;
        assert!(matches!(
            Palette::parse(&bytes),
            Err(FormatError::Invalid { offset: 10, .. })
        ));
        assert!(Palette::parse(&bytes[..700]).is_err());
    }
}
