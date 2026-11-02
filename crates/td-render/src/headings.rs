//! Sixteen-heading sprites assembled from the base sheet and the three rotation sets.
//!
//! Spec: `docs/re/FORMATS.md` §6.2. Fidelity: T0 (exact pixel transforms, no resampling).
//! The rotation sets are rotated **counter-clockwise** (measured: best overlap with the base
//! sprites at about 22.5°, 45° and 67.5° CCW for `td.sc3`, `td.shr`, `td.sc2`). So heading `h`
//! (0 = nose up, increasing counter-clockwise in 22.5° steps) uses base set
//! `h % 4` = [`td.shp`, `td.sc3`, `td.shr`, `td.sc2`] rotated counter-clockwise by
//! `90° * (h / 4)`. Every frame is normalised to a 40x40 cell; the 32x32 base sprites sit at
//! offset (4, 4). Rotation-set cells 20..23 are blank.
//!
//! How the original engine maps its internal angle to these 16 frames is U-003.

use td_formats::sprites::{BASE_SIZE, ROTATED_SIZE, SpriteSheet};

/// Number of distinct headings.
pub const HEADINGS: usize = 16;
/// Edge length of every assembled frame.
pub const CELL: usize = ROTATED_SIZE;

/// Pre-built frames for every sprite at every heading.
#[derive(Debug, Clone)]
pub struct HeadingSprites {
    /// `frames[sprite][heading]`: 40x40 row-major palette indices.
    frames: Vec<[Box<[u8]>; HEADINGS]>,
}

impl HeadingSprites {
    /// Builds all headings for the first `count` sprites of the sheets.
    ///
    /// `base` holds 32x32 frames; `rot` holds the 40x40 sets for 22.5°, 45° and 67.5° CCW.
    pub fn build(base: &SpriteSheet, rot: [&SpriteSheet; 3], count: usize) -> Option<Self> {
        let mut frames = Vec::with_capacity(count);
        for s in 0..count {
            let bases: [Box<[u8]>; 4] = [
                pad_to_cell(base.frame(s)?, BASE_SIZE),
                rot[0].frame(s)?.into(),
                rot[1].frame(s)?.into(),
                rot[2].frame(s)?.into(),
            ];
            let headings: [Box<[u8]>; HEADINGS] =
                std::array::from_fn(|h| rotate_ccw(&bases[h % 4], CELL, h / 4));
            frames.push(headings);
        }
        Some(Self { frames })
    }

    /// Number of sprites.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// True if there are no sprites.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// The 40x40 frame of `sprite` at `heading` (taken modulo 16).
    pub fn frame(&self, sprite: usize, heading: usize) -> Option<&[u8]> {
        self.frames
            .get(sprite)
            .map(|h| h[heading % HEADINGS].as_ref())
    }
}

/// Centres a `size` x `size` frame in a [`CELL`] x [`CELL`] frame filled with index 0.
fn pad_to_cell(src: &[u8], size: usize) -> Box<[u8]> {
    let off = (CELL - size) / 2;
    let mut out = vec![0u8; CELL * CELL];
    for y in 0..size {
        out[(y + off) * CELL + off..(y + off) * CELL + off + size]
            .copy_from_slice(&src[y * size..(y + 1) * size]);
    }
    out.into_boxed_slice()
}

/// Rotates a square `n` x `n` image counter-clockwise by `quarters` x 90°.
fn rotate_ccw(src: &[u8], n: usize, quarters: usize) -> Box<[u8]> {
    let mut out = vec![0u8; n * n];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = match quarters % 4 {
                0 => (x, y),
                1 => (y, n - 1 - x),
                2 => (n - 1 - x, n - 1 - y),
                _ => (n - 1 - y, x),
            };
            out[dy * n + dx] = src[y * n + x];
        }
    }
    out.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_moves_top_left_counter_clockwise() {
        // 2x2: a b / c d  ->  90° CCW: b d / a c
        let src = [1, 2, 3, 4];
        assert_eq!(&*rotate_ccw(&src, 2, 0), &[1, 2, 3, 4]);
        assert_eq!(&*rotate_ccw(&src, 2, 1), &[2, 4, 1, 3]);
        assert_eq!(&*rotate_ccw(&src, 2, 2), &[4, 3, 2, 1]);
        assert_eq!(&*rotate_ccw(&src, 2, 3), &[3, 1, 4, 2]);
    }

    #[test]
    fn builds_sixteen_headings_from_synthetic_sheets() {
        let mut base = vec![0u8; 20 * 32 * 32];
        base[0] = 7; // sprite 0, top-left pixel of the 32x32 frame
        let base = SpriteSheet::parse_base(&base).unwrap();
        let rot = SpriteSheet::parse_rotation_set(&vec![0u8; 24 * 40 * 40]).unwrap();
        let hs = HeadingSprites::build(&base, [&rot, &rot, &rot], 20).unwrap();
        assert_eq!(hs.len(), 20);
        // Heading 0: pixel at (4, 4) in the 40x40 cell. Heading 4 (90° CCW): moves to (4, 35).
        assert_eq!(hs.frame(0, 0).unwrap()[4 * 40 + 4], 7);
        assert_eq!(hs.frame(0, 4).unwrap()[35 * 40 + 4], 7);
        assert_eq!(hs.frame(0, 8).unwrap()[35 * 40 + 35], 7);
        assert_eq!(hs.frame(0, 12).unwrap()[4 * 40 + 35], 7);
        assert_eq!(hs.frame(0, 16), hs.frame(0, 0), "headings wrap");
        assert!(HeadingSprites::build(&base, [&rot, &rot, &rot], 25).is_none());
    }
}
