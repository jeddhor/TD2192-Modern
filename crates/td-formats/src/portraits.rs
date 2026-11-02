//! Dialogue portraits (`newfaces.td`, `newfaces.td2`, `newfaces.td3`).
//!
//! Spec: `docs/re/FORMATS.md` §10 (CONFIRMED by decoding). Fidelity: T0.
//! The file is a sequence of records, each a 768-byte palette followed by a 160x100 image:
//! 26 faces in episode 1 and 27 in episodes 2 and 3. Every face carries its own palette; how the
//! dialogue screen combines two faces and a backdrop in one VGA palette is part of M5-04.

use crate::palette::{PALETTE_LEN, Palette};
use crate::{FormatError, Result};

/// Portrait width in pixels.
pub const FACE_WIDTH: usize = 160;
/// Portrait height in pixels.
pub const FACE_HEIGHT: usize = 100;
/// Bytes per record (palette + pixels).
pub const RECORD_LEN: usize = PALETTE_LEN + FACE_WIDTH * FACE_HEIGHT;

/// One portrait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portrait {
    pub palette: Palette,
    /// 160x100 palette indices, row-major.
    pub pixels: Box<[u8]>,
}

/// All portraits of an episode. Dialogue speaker lines refer to them 1-based (face byte 1 is
/// `faces[0]`); larger values (32, 33, 65, 78, 87, 89, 90, 126, 200) are special codes (U-019).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portraits {
    pub faces: Vec<Portrait>,
}

impl Portraits {
    /// Parses a whole portrait file.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.is_empty() || !data.len().is_multiple_of(RECORD_LEN) {
            return Err(FormatError::Invalid {
                format: "portraits",
                offset: 0,
                detail: format!("length {} is not a multiple of {RECORD_LEN}", data.len()),
            });
        }
        let faces = data
            .chunks_exact(RECORD_LEN)
            .map(|rec| {
                Ok(Portrait {
                    palette: Palette::parse_prefix(rec)?,
                    pixels: rec[PALETTE_LEN..].into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { faces })
    }

    /// The portrait for a 1-based dialogue face byte, if it names a portrait.
    pub fn by_face_byte(&self, face: u8) -> Option<&Portrait> {
        (face >= 1)
            .then(|| self.faces.get(usize::from(face) - 1))
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_records() {
        let mut data = vec![0u8; 2 * RECORD_LEN];
        data[RECORD_LEN + PALETTE_LEN] = 9; // face 2, first pixel
        let p = Portraits::parse(&data).unwrap();
        assert_eq!(p.faces.len(), 2);
        assert_eq!(p.by_face_byte(2).unwrap().pixels[0], 9);
        assert!(p.by_face_byte(0).is_none());
        assert!(p.by_face_byte(3).is_none());
        assert!(Portraits::parse(&data[1..]).is_err());
        data[5] = 64; // invalid palette component
        assert!(Portraits::parse(&data).is_err());
    }
}
