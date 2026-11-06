//! Sixteen-heading sprites assembled from the base sheet and the three rotation sets.
//!
//! Spec: `docs/re/FORMATS.md` §6.2, `docs/spec/flow/flight-screens.md` rev 2 §4.9 (which
//! buffer each heading's blit reads). Fidelity: T0 (exact pixel transforms, no resampling).
//! The rotation sets are rotated **counter-clockwise** (measured: best overlap with the base
//! sprites at about 22.5°, 45° and 67.5° CCW for `td.sc3`, `td.shr`, `td.sc2`). Heading `h`
//! (0 = nose up, increasing counter-clockwise in 22.5° steps):
//!
//! - N, W, S, E (`h` 0, 4, 8, 12): the base sprite (bank slot) turned by 90° × `h / 4`;
//! - NW, SW, SE, NE (2, 6, 10, 14): `td.shr` turned by 90° × `h / 4`;
//! - NNW, NNE, ESE, WSW (1, 15, 11, 5) read `td.sc3` and WNW, ENE, SSE, SSW (3, 13, 9, 7) read
//!   `td.sc2` (§4.9). Among the quarter-turns and mirrors of a 40 × 40 cell only one points each
//!   of these the right way: NNW / WNW the cell itself, WSW / SSW turned 90°, NNE / ENE
//!   mirrored left to right, ESE / SSE mirrored and turned 270°. ASSUMPTION(U-360): the blits
//!   are such cell transforms; for the 0° and 45° sets a mirror would point the same way as
//!   the turn used here.
//!
//! Every frame is normalised to a 40x40 cell; the 32x32 base sprites sit at offset (4, 4).
//! Rotation-set cells 20..23 are blank.
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
            let headings: [Box<[u8]>; HEADINGS] = std::array::from_fn(|h| {
                let (set, mirrored, quarters) = heading_source(h);
                if mirrored {
                    rotate_ccw(&mirror(&bases[set], CELL), CELL, quarters)
                } else {
                    rotate_ccw(&bases[set], CELL, quarters)
                }
            });
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

/// How heading `h` (0..16) is made: the base set (0 = `td.shp`, 1 = `td.sc3`, 2 = `td.shr`,
/// 3 = `td.sc2`), whether the cell is first mirrored left to right, and the counter-clockwise
/// quarter turns (module docs; flight-screens rev 2 §4.9, U-360).
pub fn heading_source(h: usize) -> (usize, bool, usize) {
    let h = h % HEADINGS;
    match h {
        // 22.5° CCW (td.sc3): NNW, WSW turned; NNE, ESE mirrored.
        1 => (1, false, 0),
        5 => (1, false, 1),
        15 => (1, true, 0),
        11 => (1, true, 3),
        // 67.5° CCW (td.sc2): WNW, SSW turned; ENE, SSE mirrored.
        3 => (3, false, 0),
        7 => (3, false, 1),
        13 => (3, true, 0),
        9 => (3, true, 3),
        // 0° (bank) and 45° (td.shr): turned.
        _ => (h % 4, false, h / 4),
    }
}

/// Mirrors a square `n` x `n` image left to right.
fn mirror(src: &[u8], n: usize) -> Box<[u8]> {
    let mut out = vec![0u8; n * n];
    for (dst, row) in out.chunks_exact_mut(n).zip(src.chunks_exact(n)) {
        for (d, s) in dst.iter_mut().zip(row.iter().rev()) {
            *d = *s;
        }
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
    fn two_digit_headings_read_the_buffers_of_flight_screens_4_9() {
        // §4.9: td.sc3 for NNW, NNE, ESE, WSW; td.sc2 for WNW, ENE, SSE, SSW; td.shr for the
        // 45° headings; the bank for N, E, S, W (h counter-clockwise from N).
        let set = |h| heading_source(h).0;
        for (h, want) in [
            (1, 1),
            (15, 1),
            (11, 1),
            (5, 1),
            (3, 3),
            (13, 3),
            (9, 3),
            (7, 3),
        ] {
            assert_eq!(set(h), want, "h = {h}");
        }
        for h in [2, 6, 10, 14] {
            assert_eq!(heading_source(h), (2, false, h / 4));
        }
        for h in [0, 4, 8, 12] {
            assert_eq!(heading_source(h), (0, false, h / 4));
        }
    }

    #[test]
    fn every_heading_points_its_way() {
        // A one-pixel "nose" 14 pixels from the cell centre in each set's own direction
        // (td.sc3 22.5°, td.shr 45°, td.sc2 67.5° CCW) ends up within a pixel of heading h's
        // direction, h × 22.5° CCW, after the transform (mirrors included).
        let nose = |deg: f64| {
            let r = deg.to_radians();
            let x = (19.5 - 14.0 * r.sin()).round() as usize;
            let y = (19.5 - 14.0 * r.cos()).round() as usize;
            (x, y)
        };
        let mut sets: Vec<Vec<u8>> = Vec::new();
        for deg in [22.5, 45.0, 67.5] {
            let mut cells = vec![0u8; 24 * 40 * 40];
            let (x, y) = nose(deg);
            cells[y * 40 + x] = 9;
            sets.push(cells);
        }
        let mut base = vec![0u8; 20 * 32 * 32];
        let (bx, by) = nose(0.0);
        base[(by - 4) * 32 + bx - 4] = 9; // the nose of sprite 0, in its 32 x 32 frame
        let base = SpriteSheet::parse_base(&base).unwrap();
        let rot: Vec<SpriteSheet> = sets
            .iter()
            .map(|c| SpriteSheet::parse_rotation_set(c).unwrap())
            .collect();
        let hs = HeadingSprites::build(&base, [&rot[0], &rot[1], &rot[2]], 1).unwrap();
        for h in 0..HEADINGS {
            let f = hs.frame(0, h).unwrap();
            let at = f.iter().position(|&p| p == 9).unwrap();
            let (x, y) = ((at % 40) as i64, (at / 40) as i64);
            let (wx, wy) = nose(22.5 * h as f64);
            assert!(
                (x - wx as i64).abs() <= 1 && (y - wy as i64).abs() <= 1,
                "h = {h}: nose at ({x}, {y}), want about ({wx}, {wy})"
            );
        }
    }

    #[test]
    fn mirror_reverses_rows() {
        assert_eq!(&*mirror(&[1, 2, 3, 4], 2), &[2, 1, 4, 3]);
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
