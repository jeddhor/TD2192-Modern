//! Sixteen-heading sprites: the pixel transforms of the original's sixteen heading blits.
//!
//! Spec: `docs/spec/flow/flight-screens.md` rev 3 §4.9 (U-360, CONFIRMED by C-0080: every
//! heading of the player 0-diff) and `docs/re/FORMATS.md` §6.2–6.3. Fidelity: T0 (exact
//! pixel transforms, no resampling).
//!
//! Heading index `h` (0 = nose up, increasing counter-clockwise in 22.5° steps, as
//! `td_sim::flight::heading::Heading::sprite_index` numbers them) selects one of the sixteen
//! blits. Each blit draws a w × w box (w = 32 for the sprite bank, 40 for the rotation buffers
//! `td.sc3`, `td.shr`, `td.sc2`) whose top-left corner is the sprite position; box pixel
//! (c, r) takes source byte L of the sprite's cell (row-major, w × w bytes) and is written
//! unless that byte is 0:
//!
//! | `h` | Heading | Source | Source byte L for box pixel (c, r) |
//! |---:|---|---|---|
//! | 0 | N | bank | `r·w + c` |
//! | 1, 2, 3 | NNW, NW, WNW | `td.sc3`, `td.shr`, `td.sc2` | `r·40 + c` |
//! | 4 | W | bank | `c·w + (w − r)` (quarter turn anticlockwise, one row down) |
//! | 5, 6, 7 | WSW, SW, SSW | `td.sc3`, `td.shr`, `td.sc2` | `c·40 + (40 − r)` |
//! | 8 | S | bank | `(w − 1 − r)·w + c + 1` (top-bottom mirror, one column left) |
//! | 9, 10, 11 | SSE, SE, ESE | `td.sc2`, `td.shr`, `td.sc3` | `(39 − c)·40 + (40 − r)` |
//! | 12 | E | bank | `(w − 1 − c)·w + (w − r)` (anti-transpose, one row down) |
//! | 13, 14, 15 | ENE, NE, NNE | `td.sc2`, `td.shr`, `td.sc3` | `r·40 + (39 − c)` (left-right mirror) |
//!
//! So E, S, NE, SE, NNE, ESE, ENE and SSE are mirror images, not rotations, and the blits of W,
//! WSW, SW, SSW, S, SSE, SE, ESE and E read one byte further on than the plain transform (the
//! table's nine; the spec's prose says ten): L can reach w·w, the first byte of whatever
//! follows the cell in memory. [`HeadingSprites::from_buffers`] models the flight's memory: the
//! four buffers lie back to back as `td.sc3` (cells 0..8), `td.sc2` (cells 0..8), `td.shr`
//! (cells 0..19), then the 27-slot bank (flight-screens §4.9 loads and §9 provenance: buffers
//! at 633D, 66C1, 6A45, bank at 7215, each starting where the one before ends), so cell 8 of
//! `td.sc3` is followed by the first byte of `td.sc2`, and so on.
//!
//! Every frame is stored as [`CELL`] × [`CELL`] (40 × 40) with the heading's box at its
//! top-left corner and zeros elsewhere: draw it with its top-left corner at the sprite
//! position. The 40 × 40 boxes are **not** centred on the 32 × 32 position, so a ship drawn on
//! a rotated heading appears about 4 pixels right of and below where it appears on N, E, S, W.

use td_formats::sprites::{BASE_SIZE, ROTATED_SIZE, SpriteSheet};

/// Number of distinct headings.
pub const HEADINGS: usize = 16;
/// Edge length of every assembled frame (the largest box).
pub const CELL: usize = ROTATED_SIZE;

/// Rotation cells of `td.sc3` and `td.sc2` the flight loads (cells 0..8, 14,400 bytes each;
/// FORMATS §6.3, flight-screens §4.9).
pub const FLIGHT_SC3_SC2_CELLS: usize = 9;
/// Rotation cells of `td.shr` the flight loads (cells 0..19, 32,000 bytes).
pub const FLIGHT_SHR_CELLS: usize = 20;
/// Slots of the flight's sprite bank (`td.shp` 0..19, then `td.exp` 0..6).
pub const FLIGHT_BANK_SLOTS: usize = 27;

/// The buffer a heading's blit reads (flight-screens §4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The 32 × 32 sprite bank (`td.shp`, `td.exp`).
    Bank,
    /// `td.sc3` (22.5° anticlockwise).
    Sc3,
    /// `td.shr` (45°).
    Shr,
    /// `td.sc2` (67.5°).
    Sc2,
}

impl Source {
    /// Edge length of this buffer's cells.
    #[must_use]
    pub fn size(self) -> usize {
        match self {
            Source::Bank => BASE_SIZE,
            _ => ROTATED_SIZE,
        }
    }
}

/// The pixel transform of a heading's blit (flight-screens §4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    /// The cell as it is: `r·w + c`.
    Plain,
    /// Left-right mirror: `r·w + (w − 1 − c)`.
    MirrorLeftRight,
    /// Quarter turn anticlockwise, one row down: `c·w + (w − r)`.
    QuarterTurnDown,
    /// Mirror about the anti-diagonal, one row down: `(w − 1 − c)·w + (w − r)`.
    AntiTransposeDown,
    /// Top-bottom mirror, one column left: `(w − 1 − r)·w + c + 1`.
    MirrorTopBottomLeft,
}

impl Transform {
    /// Source byte L of box pixel (`c`, `r`) of a `w` × `w` box. L ranges over 0..=w·w; w·w is
    /// the first byte after the cell.
    #[must_use]
    pub fn source_index(self, w: usize, c: usize, r: usize) -> usize {
        match self {
            Transform::Plain => r * w + c,
            Transform::MirrorLeftRight => r * w + (w - 1 - c),
            Transform::QuarterTurnDown => c * w + (w - r),
            Transform::AntiTransposeDown => (w - 1 - c) * w + (w - r),
            Transform::MirrorTopBottomLeft => (w - 1 - r) * w + c + 1,
        }
    }
}

/// The blit of heading `h` (taken modulo 16): its source buffer and transform
/// (flight-screens rev 3 §4.9 table; module docs).
#[must_use]
pub fn blit(h: usize) -> (Source, Transform) {
    use Source::{Bank, Sc2, Sc3, Shr};
    use Transform::QuarterTurnDown as Quarter;
    use Transform::{AntiTransposeDown, MirrorLeftRight, MirrorTopBottomLeft, Plain};
    match h % HEADINGS {
        0 => (Bank, Plain),               // N (8), 386E:024A
        1 => (Sc3, Plain),                // NNW (4E), 0D68
        2 => (Shr, Plain),                // NW (7), 0391
        3 => (Sc2, Plain),                // WNW (4A), 1302
        4 => (Bank, Quarter),             // W (4), 0926
        5 => (Sc3, Quarter),              // WSW (0E), 1193
        6 => (Shr, Quarter),              // SW (1), 0A93
        7 => (Sc2, Quarter),              // SSW (0C), 172D
        8 => (Bank, MirrorTopBottomLeft), // S (2), 0C02
        9 => (Sc2, AntiTransposeDown),    // SSE (20), 15C1
        10 => (Shr, AntiTransposeDown),   // SE (3), 0650
        11 => (Sc3, AntiTransposeDown),   // ESE (24), 1027
        12 => (Bank, AntiTransposeDown),  // E (6), 07BC
        13 => (Sc2, MirrorLeftRight),     // ENE (60), 145A
        14 => (Shr, MirrorLeftRight),     // NE (9), 04E9
        _ => (Sc3, MirrorLeftRight),      // NNE (62), 0EC0
    }
}

/// Edge length of heading `h`'s box: 32 for N, W, S, E, 40 for the others.
#[must_use]
pub fn box_size(h: usize) -> usize {
    blit(h).0.size()
}

/// The four sprite buffers the heading blits read, in their memory order: `td.sc3`, `td.sc2`,
/// `td.shr` (40 × 40 cells), then the bank (32 × 32 slots). A one-pixel shift that reads past
/// a buffer's last cell finds the first byte of the next buffer (or 0 after the bank).
#[derive(Debug, Clone, Copy)]
pub struct SpriteBuffers<'a> {
    pub sc3: &'a [u8],
    pub sc2: &'a [u8],
    pub shr: &'a [u8],
    pub bank: &'a [u8],
}

impl<'a> SpriteBuffers<'a> {
    /// The buffers as the flight loads them (flight-screens §4.9, FORMATS §6.3): `td.sc3` and
    /// `td.sc2` up to cell 8, `td.shr` up to cell 19, the bank's 27 slots. Longer inputs are
    /// cut to those lengths; shorter ones are used as they are.
    #[must_use]
    pub fn flight(sc3: &'a [u8], sc2: &'a [u8], shr: &'a [u8], bank: &'a [u8]) -> Self {
        let cut = |b: &'a [u8], cells: usize, size: usize| &b[..b.len().min(cells * size * size)];
        Self {
            sc3: cut(sc3, FLIGHT_SC3_SC2_CELLS, ROTATED_SIZE),
            sc2: cut(sc2, FLIGHT_SC3_SC2_CELLS, ROTATED_SIZE),
            shr: cut(shr, FLIGHT_SHR_CELLS, ROTATED_SIZE),
            bank: cut(bank, FLIGHT_BANK_SLOTS, BASE_SIZE),
        }
    }

    /// The buffers laid back to back.
    fn memory(&self) -> Vec<u8> {
        [self.sc3, self.sc2, self.shr, self.bank].concat()
    }

    /// Byte offset of `source`'s first cell in [`SpriteBuffers::memory`].
    fn offset(&self, source: Source) -> usize {
        match source {
            Source::Sc3 => 0,
            Source::Sc2 => self.sc3.len(),
            Source::Shr => self.sc3.len() + self.sc2.len(),
            Source::Bank => self.sc3.len() + self.sc2.len() + self.shr.len(),
        }
    }
}

/// Pre-built frames for every sprite at every heading.
#[derive(Debug, Clone)]
pub struct HeadingSprites {
    /// `frames[sprite][heading]`: 40 × 40 row-major palette indices, the box at the top left.
    frames: Vec<[Box<[u8]>; HEADINGS]>,
}

impl HeadingSprites {
    /// Builds all headings for the first `count` sprites from whole sheets: `base` holds the
    /// 32 × 32 frames, `rot` the 40 × 40 sets for 22.5°, 45° and 67.5° (`td.sc3`, `td.shr`,
    /// `td.sc2`). The sheets are laid out in the flight's memory order, uncut (a viewer of the
    /// files; the flight uses [`HeadingSprites::from_buffers`] with
    /// [`SpriteBuffers::flight`]). `None` if a sheet has fewer than `count` frames.
    pub fn build(base: &SpriteSheet, rot: [&SpriteSheet; 3], count: usize) -> Option<Self> {
        if [base, rot[0], rot[1], rot[2]]
            .iter()
            .any(|s| s.len() < count)
        {
            return None;
        }
        let bytes = |s: &SpriteSheet| -> Vec<u8> {
            (0..s.len())
                .flat_map(|i| s.frame(i).unwrap_or_default().to_vec())
                .collect()
        };
        let (sc3, shr, sc2, bank) = (bytes(rot[0]), bytes(rot[1]), bytes(rot[2]), bytes(base));
        let buffers = SpriteBuffers {
            sc3: &sc3,
            sc2: &sc2,
            shr: &shr,
            bank: &bank,
        };
        Some(Self::from_buffers(&buffers, count))
    }

    /// Builds all headings for sprites 0..`count` from the buffers as they lie in memory
    /// (module docs). Sprite n of a buffer is the n-th cell from that buffer's start, even
    /// past its end (the original does not check: sprite 9 on a `td.sc3` heading would read
    /// `td.sc2` cell 0); bytes beyond the last buffer read as 0.
    #[must_use]
    pub fn from_buffers(buffers: &SpriteBuffers<'_>, count: usize) -> Self {
        let mem = buffers.memory();
        let byte = |i: usize| mem.get(i).copied().unwrap_or(0);
        let frames = (0..count)
            .map(|s| {
                std::array::from_fn(|h| {
                    let (source, transform) = blit(h);
                    let w = source.size();
                    let base = buffers.offset(source) + s * w * w;
                    let mut out = vec![0u8; CELL * CELL];
                    for r in 0..w {
                        for c in 0..w {
                            out[r * CELL + c] = byte(base + transform.source_index(w, c, r));
                        }
                    }
                    out.into_boxed_slice()
                })
            })
            .collect();
        Self { frames }
    }

    /// Number of sprites.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// True if there are no sprites.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// The 40 × 40 frame of `sprite` at `heading` (taken modulo 16): the heading's box at the
    /// top-left corner, zeros elsewhere.
    pub fn frame(&self, sprite: usize, heading: usize) -> Option<&[u8]> {
        self.frames
            .get(sprite)
            .map(|h| h[heading % HEADINGS].as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Buffers whose bytes are their own memory index + 1 (mod 251, never 0), so every pixel
    /// says where it was read from.
    fn numbered(cells: [usize; 4]) -> Vec<Vec<u8>> {
        let sizes = [40 * 40, 40 * 40, 40 * 40, 32 * 32];
        let mut n = 0usize;
        cells
            .iter()
            .zip(sizes)
            .map(|(&k, sz)| {
                (0..k * sz)
                    .map(|_| {
                        n += 1;
                        (n % 251) as u8 + 1
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn source_indices_follow_the_spec_table() {
        // flight-screens rev 3 §4.9, w = 32 and 40: corners and the one-byte overrun.
        for w in [32, 40] {
            let last = w - 1;
            assert_eq!(Transform::Plain.source_index(w, 0, 0), 0);
            assert_eq!(Transform::Plain.source_index(w, last, last), w * w - 1);
            assert_eq!(Transform::MirrorLeftRight.source_index(w, 0, 0), last);
            // Quarter turn: box row 0 takes source column 0 one row further on; (0, 0) reads
            // row 1, column 0 and (w−1, 0) the first byte after the cell.
            assert_eq!(Transform::QuarterTurnDown.source_index(w, 0, 0), w);
            assert_eq!(Transform::QuarterTurnDown.source_index(w, last, 0), w * w);
            assert_eq!(Transform::QuarterTurnDown.source_index(w, 0, 1), last);
            assert_eq!(Transform::AntiTransposeDown.source_index(w, 0, 0), w * w);
            assert_eq!(Transform::AntiTransposeDown.source_index(w, last, last), 1);
            assert_eq!(
                Transform::MirrorTopBottomLeft.source_index(w, 0, 0),
                last * w + 1
            );
            assert_eq!(
                Transform::MirrorTopBottomLeft.source_index(w, last, 0),
                w * w
            );
            assert_eq!(Transform::MirrorTopBottomLeft.source_index(w, 0, last), 1);
        }
    }

    #[test]
    fn heading_buffers_match_flight_screens_4_9() {
        // Bank for N, W, S, E; td.shr for the 45° headings; td.sc3 for NNW, WSW, ESE, NNE;
        // td.sc2 for WNW, SSW, SSE, ENE (h counter-clockwise from N).
        for (h, src) in [
            (0, Source::Bank),
            (4, Source::Bank),
            (8, Source::Bank),
            (12, Source::Bank),
            (2, Source::Shr),
            (6, Source::Shr),
            (10, Source::Shr),
            (14, Source::Shr),
            (1, Source::Sc3),
            (5, Source::Sc3),
            (11, Source::Sc3),
            (15, Source::Sc3),
            (3, Source::Sc2),
            (7, Source::Sc2),
            (9, Source::Sc2),
            (13, Source::Sc2),
        ] {
            assert_eq!(blit(h).0, src, "h = {h}");
            assert_eq!(box_size(h), if src == Source::Bank { 32 } else { 40 });
        }
        // The eight mirrored headings: E, S, NE, SE, NNE, ESE, ENE, SSE.
        let mirrored: Vec<usize> = (0..16)
            .filter(|&h| {
                matches!(
                    blit(h).1,
                    Transform::MirrorLeftRight
                        | Transform::AntiTransposeDown
                        | Transform::MirrorTopBottomLeft
                )
            })
            .collect();
        assert_eq!(mirrored, [8, 9, 10, 11, 12, 13, 14, 15]);
        // The blits that shift by one pixel: E, W, SW, SE, ESE, WSW, SSE, SSW (down) and S
        // (left). The §4.9 table lists these nine; its prose says "ten" (reported as a spec
        // erratum).
        let shifted: Vec<usize> = (0..16)
            .filter(|&h| !matches!(blit(h).1, Transform::Plain | Transform::MirrorLeftRight))
            .collect();
        assert_eq!(shifted, [4, 5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(blit(16), blit(0), "headings wrap");
    }

    #[test]
    fn frames_read_the_buffers_back_to_back() {
        // Two cells of each rotation buffer and three bank slots, numbered by memory index.
        let b = numbered([2, 2, 2, 3]);
        let bufs = SpriteBuffers {
            sc3: &b[0],
            sc2: &b[1],
            shr: &b[2],
            bank: &b[3],
        };
        let hs = HeadingSprites::from_buffers(&bufs, 2);
        let mem: Vec<u8> = b.concat();
        let (sc2, bank) = (2 * 1600, 6 * 1600);
        for s in 0..2 {
            for h in 0..HEADINGS {
                let (src, t) = blit(h);
                let w = src.size();
                let base = bufs.offset(src) + s * w * w;
                let f = hs.frame(s, h).unwrap();
                for r in 0..CELL {
                    for c in 0..CELL {
                        let want = if r < w && c < w {
                            mem.get(base + t.source_index(w, c, r))
                                .copied()
                                .unwrap_or(0)
                        } else {
                            0
                        };
                        assert_eq!(f[r * CELL + c], want, "sprite {s} h {h} ({c}, {r})");
                    }
                }
            }
        }
        // Sprite 1 on NNE-shifted ESE (h 11) reads past td.sc3's last cell into td.sc2 byte 0.
        let ese = hs.frame(1, 11).unwrap();
        assert_eq!(ese[0], mem[sc2]);
        // Sprite 1 on SE reads past td.shr into the bank's first byte; on E past slot 1 into
        // slot 2.
        assert_eq!(hs.frame(1, 10).unwrap()[0], mem[bank]);
        assert_eq!(hs.frame(1, 12).unwrap()[0], mem[bank + 2 * 1024]);
        // Sprite 1 of S: box column 31 of row 0 is the first byte of slot 2.
        assert_eq!(hs.frame(1, 8).unwrap()[31], mem[bank + 2 * 1024]);
    }

    #[test]
    fn flight_buffers_are_cut_to_the_loaded_cells() {
        let big = vec![1u8; 24 * 1600];
        let bank = vec![2u8; 30 * 1024];
        let f = SpriteBuffers::flight(&big, &big, &big, &bank);
        assert_eq!(
            (f.sc3.len(), f.sc2.len(), f.shr.len(), f.bank.len()),
            (14_400, 14_400, 32_000, 27 * 1024)
        );
        // Sprite 9 on NNW (td.sc3 plain) reads td.sc2 cell 0 in the flight's memory.
        let mut sc2 = vec![0u8; 24 * 1600];
        sc2[0] = 77;
        let f = SpriteBuffers::flight(&big, &sc2, &big, &bank);
        let hs = HeadingSprites::from_buffers(&f, 10);
        assert_eq!(hs.frame(9, 1).unwrap()[0], 77);
    }

    #[test]
    fn every_heading_points_its_way() {
        // A one-pixel "nose" 14 pixels from the cell centre (16 for the bank, 20 for the
        // rotation cells) in each set's own direction ends up within two pixels of heading h's
        // direction, h × 22.5° anticlockwise, after the transform (mirrors and the one-pixel
        // shifts included).
        let nose = |deg: f64, centre: f64| {
            let r = deg.to_radians();
            let x = (centre - 0.5 - 14.0 * r.sin()).round() as usize;
            let y = (centre - 0.5 - 14.0 * r.cos()).round() as usize;
            (x, y)
        };
        let mut sets: Vec<Vec<u8>> = Vec::new();
        for deg in [22.5, 45.0, 67.5] {
            let mut cells = vec![0u8; 24 * 40 * 40];
            let (x, y) = nose(deg, 20.0);
            cells[y * 40 + x] = 9;
            sets.push(cells);
        }
        let mut base = vec![0u8; 20 * 32 * 32];
        let (bx, by) = nose(0.0, 16.0);
        base[by * 32 + bx] = 9;
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
            let (wx, wy) = nose(22.5 * h as f64, box_size(h) as f64 / 2.0);
            assert!(
                (x - wx as i64).abs() <= 2 && (y - wy as i64).abs() <= 2,
                "h = {h}: nose at ({x}, {y}), want about ({wx}, {wy})"
            );
        }
    }

    #[test]
    fn bank_headings_keep_the_box_at_the_corner() {
        let mut base = vec![0u8; 20 * 32 * 32];
        base[0] = 7; // sprite 0, top-left pixel
        base[32 * 32 - 1] = 5; // sprite 0, bottom-right pixel
        let base = SpriteSheet::parse_base(&base).unwrap();
        let rot = SpriteSheet::parse_rotation_set(&vec![0u8; 24 * 40 * 40]).unwrap();
        let hs = HeadingSprites::build(&base, [&rot, &rot, &rot], 20).unwrap();
        assert_eq!(hs.len(), 20);
        let at = |h: usize, v: u8| {
            let f = hs.frame(0, h).unwrap();
            let i = f.iter().position(|&p| p == v).unwrap();
            (i % 40, i / 40)
        };
        // N: as it is, in the 32 × 32 corner of the frame.
        assert_eq!((at(0, 7), at(0, 5)), ((0, 0), (31, 31)));
        // W (quarter turn anticlockwise, one row down): source (0, 0) at box (0, 32) is cut
        // off; source (31, 31) moves to box (31, 1).
        assert_eq!(at(4, 5), (31, 1));
        assert!(!hs.frame(0, 4).unwrap().contains(&7));
        // S (top-bottom mirror, one column left): source (31, 31) at box (30, 0); source
        // (0, 0) falls off the left edge.
        assert_eq!(at(8, 5), (30, 0));
        assert!(!hs.frame(0, 8).unwrap().contains(&7));
        // E (anti-transpose, one row down): source (31, 31) at box (0, 1).
        assert_eq!(at(12, 5), (0, 1));
        assert_eq!(hs.frame(0, 16), hs.frame(0, 0), "headings wrap");
        assert!(HeadingSprites::build(&base, [&rot, &rot, &rot], 25).is_none());
    }
}
