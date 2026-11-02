//! Headerless sprite sheets: consecutive square frames of 8bpp pixels.
//!
//! Spec: `docs/re/FORMATS.md` §6. Fidelity: T0.
//! * `td.shp`: 20 frames of 32x32 at heading 0 (CONFIRMED).
//! * `td.sc3`, `td.shr`, `td.sc2`: 24 cells of 40x40 holding the same sprites pre-rotated by
//!   about 22.5°, 45° and 67.5° (LIKELY).
//! * `mine.td` (1 x 32x32), `lander.td` (5 x 32x32), `td.exp` (7 x 32x32) (HYPOTHESIS).

use crate::{FormatError, Result};

/// Edge length of base sprites (`td.shp`).
pub const BASE_SIZE: usize = 32;
/// Number of frames in `td.shp`.
pub const BASE_FRAMES: usize = 20;
/// Edge length of rotation-set cells.
pub const ROTATED_SIZE: usize = 40;
/// Number of cells in each rotation set.
pub const ROTATED_CELLS: usize = 24;

/// A sheet of square frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteSheet {
    size: usize,
    frames: Vec<Box<[u8]>>,
}

impl SpriteSheet {
    /// Parses `data` as whole `size` x `size` frames; the length must be an exact multiple.
    pub fn parse(data: &[u8], size: usize) -> Result<Self> {
        let frame = size * size;
        if size == 0 || data.is_empty() || !data.len().is_multiple_of(frame) {
            return Err(FormatError::Invalid {
                format: "sprite sheet",
                offset: 0,
                detail: format!("length {} is not a multiple of {size}x{size}", data.len()),
            });
        }
        Ok(Self {
            size,
            frames: data.chunks_exact(frame).map(Into::into).collect(),
        })
    }

    /// Parses `td.shp` (20 frames of 32x32).
    pub fn parse_base(data: &[u8]) -> Result<Self> {
        FormatError::check_size("td.shp", data, BASE_FRAMES * BASE_SIZE * BASE_SIZE)?;
        Self::parse(data, BASE_SIZE)
    }

    /// Parses a rotation set (24 cells of 40x40).
    pub fn parse_rotation_set(data: &[u8]) -> Result<Self> {
        FormatError::check_size(
            "rotation set",
            data,
            ROTATED_CELLS * ROTATED_SIZE * ROTATED_SIZE,
        )?;
        Self::parse(data, ROTATED_SIZE)
    }

    /// Frame edge length in pixels.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Number of frames.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// True if the sheet has no frames (never the case for a parsed sheet).
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Pixels of frame `index`, row-major.
    pub fn frame(&self, index: usize) -> Option<&[u8]> {
        self.frames.get(index).map(AsRef::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_frames() {
        let mut data = vec![0u8; 20 * 1024];
        data[1024] = 5;
        let s = SpriteSheet::parse_base(&data).unwrap();
        assert_eq!((s.len(), s.size()), (20, 32));
        assert_eq!(s.frame(1).unwrap()[0], 5);
        assert!(s.frame(20).is_none());
        assert!(SpriteSheet::parse_rotation_set(&data).is_err());
        assert!(SpriteSheet::parse(&data[..1000], 32).is_err());
        assert!(SpriteSheet::parse(&[], 32).is_err());
        assert_eq!(
            SpriteSheet::parse_rotation_set(&vec![0u8; 38_400])
                .unwrap()
                .len(),
            24
        );
    }
}
