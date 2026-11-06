//! Pixel-art upscalers (`docs/spec/enhanced/presentation.md` rev 1 §5, M12-10).
//!
//! - **Scale2x** (EPX / AdvMAME2x) with the rule set of §5; **Scale3x** is the AdvMAME3x rule
//!   set; **Scale4x** is Scale2x applied twice.
//! - **xBR level 2** at 2×, 3× and 4× (Hyllian's public algorithm, implemented from its published
//!   description):
//!   - For each corner of a pixel E, an edge runs across the corner when the weighted sum of YUV
//!     distances along the corner's diagonal beats the sum across it (weights 1, 1, 1, 1 and 4
//!     for the central pair).
//!   - The corner then blends towards the closer of its two side neighbours.
//!   - Level 2 recognises shallow and steep edges (2:1 slopes) by comparing the distances F–G
//!     and H–C with a factor of 2, and widens the blend along them.
//!   - Distances are 48·|ΔY| + 7·|ΔU| + 6·|ΔV|.
//!   - The blend weight of each output sub-pixel is the area of it that lies beyond the edge
//!     line. The lines run through (1, ½)–(½, 1) for a diagonal edge, (0, 1)–(1, ½) for a
//!     shallow one and (1, 0)–(½, 1) for a steep one, in a unit pixel with the corner at (1, 1).
//!     The weights are computed exactly, once per factor. They give the published 2× weights
//!     (½ diagonal; ¾ and ¼ for single slopes) and 3× weights (⅞ and ⅛ diagonal; 1, ¾ and ¼ for
//!     single slopes). An edge that is both shallow and steep takes the larger weight of the two
//!     per sub-pixel.
//!   - Corners are applied in a fixed order (bottom-right, bottom-left, top-right, top-left),
//!     each blending into the sub-pixels as the earlier corners left them.
//!
//! **Edges:** pixels outside the image repeat the nearest edge pixel. An optional split row
//! (row 160 in flight frames, between playfield and HUD) is a hard edge: each side is filtered
//! as if it were a separate image.
//!
//! Output rows are computed in parallel with `rayon`. Every output pixel depends only on the
//! input, so the result is the same for any thread count.

use std::sync::OnceLock;

use rayon::prelude::*;

/// One RGBA8 pixel.
type Px = [u8; 4];

/// The upscaling filter (`upscaler = ...`, spec §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Upscaler {
    #[default]
    None,
    Scale2x,
    Scale3x,
    Scale4x,
    Xbr2,
    Xbr3,
    Xbr4,
}

impl Upscaler {
    /// Every filter, in the order the settings list them.
    pub const ALL: [Upscaler; 7] = [
        Upscaler::None,
        Upscaler::Scale2x,
        Upscaler::Scale3x,
        Upscaler::Scale4x,
        Upscaler::Xbr2,
        Upscaler::Xbr3,
        Upscaler::Xbr4,
    ];

    /// The scale factor k.
    pub fn factor(self) -> usize {
        match self {
            Upscaler::None => 1,
            Upscaler::Scale2x | Upscaler::Xbr2 => 2,
            Upscaler::Scale3x | Upscaler::Xbr3 => 3,
            Upscaler::Scale4x | Upscaler::Xbr4 => 4,
        }
    }

    /// The next filter in [`Self::ALL`], wrapping (developer hotkey).
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&u| u == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Intermediate buffers kept between frames.
#[derive(Debug, Default)]
pub struct Scratch {
    /// Scale4x's 2× intermediate image.
    mid: Vec<Px>,
    /// xBR's padded input.
    padded: Padded,
}

/// Upscales `src` (`size` = (w, h), row-major RGBA8) by `kind`'s factor into `dst`, which is
/// resized to (w·k) × (h·k). `split` is a row that is filtered as a hard edge.
///
/// # Panics
/// If `src` does not hold w × h pixels, or for [`Upscaler::None`].
pub fn upscale(
    src: &[Px],
    (w, h): (usize, usize),
    split: Option<usize>,
    kind: Upscaler,
    dst: &mut Vec<Px>,
    scratch: &mut Scratch,
) {
    assert_eq!(src.len(), w * h, "upscale input size");
    let img = Src::new(src, w, h, split);
    match kind {
        Upscaler::None => panic!("Upscaler::None does not upscale"),
        Upscaler::Scale2x => run(&img, 2, dst, scale2x_row),
        Upscaler::Scale3x => run(&img, 3, dst, scale3x_row),
        Upscaler::Scale4x => {
            run(&img, 2, &mut scratch.mid, scale2x_row);
            let mid = Src::new(&scratch.mid, 2 * w, 2 * h, split.map(|s| 2 * s));
            run(&mid, 2, dst, scale2x_row);
        }
        Upscaler::Xbr2 | Upscaler::Xbr3 | Upscaler::Xbr4 => {
            let k = kind.factor();
            scratch.padded.build(&img);
            let table = &coverage_tables()[k - 2];
            let padded = &scratch.padded;
            run(&img, k, dst, |img, y, out| {
                xbr_row(padded, img, k, table, y, out)
            });
        }
    }
}

/// xBR's input: each band of the image with a 2-pixel border of repeated edge pixels, its YUV,
/// and the distances between every pixel and its right, lower, lower-right neighbours and the
/// anti-diagonal pair right/lower, so that the edge rules read neighbours and distances at
/// fixed offsets.
#[derive(Debug, Default)]
struct Padded {
    /// Padded width w + 4.
    pw: usize,
    px: Vec<Px>,
    yuv: Vec<[i32; 3]>,
    /// d(p, p + (1, 1)).
    dm: Vec<u32>,
    /// d(p + (1, 0), p + (0, 1)).
    da: Vec<u32>,
    /// d(p, p + (1, 0)).
    dh: Vec<u32>,
    /// d(p, p + (0, 1)).
    dv: Vec<u32>,
    /// Per band: first image row, end row, index of the band's padded row 0.
    bands: Vec<(usize, usize, usize)>,
    /// Per padded row: the image row it repeats, and whether the band has a row below it.
    rows: Vec<(usize, bool)>,
}

impl Padded {
    fn build(&mut self, img: &Src<'_>) {
        let pw = img.w + 4;
        self.pw = pw;
        self.bands.clear();
        self.rows.clear();
        let mut base = 0;
        let edges = match img.split {
            Some(s) => [(0, s), (s, img.h)].to_vec(),
            None => [(0, img.h)].to_vec(),
        };
        for (lo, hi) in edges {
            self.bands.push((lo, hi, base));
            let rows = hi - lo + 4;
            for pr in 0..rows {
                let y = (lo + pr).saturating_sub(2).clamp(lo, hi - 1);
                self.rows.push((y, pr + 1 < rows));
            }
            base += rows * pw;
        }
        let n = base;
        self.px.resize(n, [0; 4]);
        self.yuv.resize(n, [0; 3]);
        for v in [&mut self.dm, &mut self.da, &mut self.dh, &mut self.dv] {
            v.resize(n, 0);
        }
        let rows = &self.rows;
        self.px
            .par_chunks_mut(pw)
            .zip(self.yuv.par_chunks_mut(pw))
            .zip(rows.par_iter())
            .for_each(|((px, yv), &(y, _))| {
                for (pc, (p, q)) in px.iter_mut().zip(yv.iter_mut()).enumerate() {
                    *p = img.px[y * img.w + pc.saturating_sub(2).min(img.w - 1)];
                    *q = yuv(*p);
                }
            });
        let yv = &self.yuv;
        self.dm
            .par_chunks_mut(pw)
            .zip(self.da.par_chunks_mut(pw))
            .zip(self.dh.par_chunks_mut(pw))
            .zip(self.dv.par_chunks_mut(pw))
            .enumerate()
            .for_each(|(r, (((dm, da), dh), dv))| {
                // The last row and column of each band have no pair partner; they are never
                // read.
                let has_next = rows[r].1;
                for pc in 0..pw {
                    let i = r * pw + pc;
                    let inner = has_next && pc + 1 < pw;
                    dm[pc] = if inner {
                        dist(yv[i], yv[i + pw + 1])
                    } else {
                        0
                    };
                    da[pc] = if inner {
                        dist(yv[i + 1], yv[i + pw])
                    } else {
                        0
                    };
                    dh[pc] = if inner { dist(yv[i], yv[i + 1]) } else { 0 };
                    dv[pc] = if inner { dist(yv[i], yv[i + pw]) } else { 0 };
                }
            });
    }

    /// Padded index of image row `y`'s first pixel.
    fn row(&self, y: usize) -> usize {
        let &(lo, _, base) = self
            .bands
            .iter()
            .find(|&&(lo, hi, _)| (lo..hi).contains(&y))
            .expect("row inside the image");
        base + (y - lo + 2) * self.pw + 2
    }

    /// The distance between the pixels at offsets `a` and `b` from index `e`, from the
    /// distance maps when they are neighbours.
    #[inline(always)]
    fn d(&self, e: usize, a: (isize, isize), b: (isize, isize)) -> u32 {
        let pw = self.pw as isize;
        let at = |(x, y): (isize, isize)| (e as isize + x + y * pw) as usize;
        match (b.0 - a.0, b.1 - a.1) {
            (1, 1) => self.dm[at(a)],
            (-1, -1) => self.dm[at(b)],
            (1, -1) => self.da[at((a.0, a.1 - 1))],
            (-1, 1) => self.da[at((b.0, b.1 - 1))],
            (1, 0) => self.dh[at(a)],
            (-1, 0) => self.dh[at(b)],
            (0, 1) => self.dv[at(a)],
            (0, -1) => self.dv[at(b)],
            _ => dist(self.yuv[at(a)], self.yuv[at(b)]),
        }
    }
}

/// The input image with its edge rules.
#[derive(Debug, Clone, Copy)]
struct Src<'a> {
    px: &'a [Px],
    w: usize,
    h: usize,
    split: Option<usize>,
}

impl<'a> Src<'a> {
    fn new(px: &'a [Px], w: usize, h: usize, split: Option<usize>) -> Self {
        let split = split.filter(|&s| s > 0 && s < h);
        Self { px, w, h, split }
    }

    /// Start index of the row `dy` rows from `y`, clamped to `y`'s band.
    fn row(&self, y: usize, dy: isize) -> usize {
        let (lo, hi) = match self.split {
            Some(s) if y < s => (0, s),
            Some(s) => (s, self.h),
            None => (0, self.h),
        };
        let r = (y as isize + dy).clamp(lo as isize, hi as isize - 1) as usize;
        r * self.w
    }

    /// Column `dx` from `x`, clamped to the image.
    fn col(&self, x: usize, dx: isize) -> usize {
        (x as isize + dx).clamp(0, self.w as isize - 1) as usize
    }
}

/// Fills `dst` ((w·k) × (h·k)) with `row(img, y, out)` for every input row y, where `out` is
/// the k output rows of row y.
fn run<F>(img: &Src<'_>, k: usize, dst: &mut Vec<Px>, row: F)
where
    F: Fn(&Src<'_>, usize, &mut [Px]) + Sync,
{
    let n = img.w * k * img.h * k;
    if dst.len() != n {
        dst.resize(n, [0, 0, 0, 255]);
    }
    dst.par_chunks_mut(img.w * k * k)
        .enumerate()
        .for_each(|(y, out)| row(img, y, out));
}

/// Scale2x (§5): E0..E3 from B, D, F, H.
fn scale2x_row(img: &Src<'_>, y: usize, out: &mut [Px]) {
    let (up, mid, down) = (img.row(y, -1), img.row(y, 0), img.row(y, 1));
    let ow = 2 * img.w;
    for x in 0..img.w {
        let p = |r: usize, dx: isize| img.px[r + img.col(x, dx)];
        let (b, d, e, f, h) = (p(up, 0), p(mid, -1), p(mid, 0), p(mid, 1), p(down, 0));
        out[2 * x] = if d == b && b != f && d != h { d } else { e };
        out[2 * x + 1] = if b == f && b != d && f != h { f } else { e };
        out[ow + 2 * x] = if d == h && d != b && h != f { d } else { e };
        out[ow + 2 * x + 1] = if h == f && d != h && b != f { f } else { e };
    }
}

/// Scale3x (AdvMAME3x): with A B C / D E F / G H I around E, and only where B ≠ H and D ≠ F.
fn scale3x_row(img: &Src<'_>, y: usize, out: &mut [Px]) {
    let (up, mid, down) = (img.row(y, -1), img.row(y, 0), img.row(y, 1));
    let ow = 3 * img.w;
    for x in 0..img.w {
        let p = |r: usize, dx: isize| img.px[r + img.col(x, dx)];
        let (a, b, c) = (p(up, -1), p(up, 0), p(up, 1));
        let (d, e, f) = (p(mid, -1), p(mid, 0), p(mid, 1));
        let (g, h, i) = (p(down, -1), p(down, 0), p(down, 1));
        let cell = if b != h && d != f {
            [
                if d == b { d } else { e },
                if (d == b && e != c) || (b == f && e != a) {
                    b
                } else {
                    e
                },
                if b == f { f } else { e },
                if (d == b && e != g) || (d == h && e != a) {
                    d
                } else {
                    e
                },
                e,
                if (b == f && e != i) || (h == f && e != c) {
                    f
                } else {
                    e
                },
                if d == h { d } else { e },
                if (d == h && e != i) || (h == f && e != g) {
                    h
                } else {
                    e
                },
                if h == f { f } else { e },
            ]
        } else {
            [e; 9]
        };
        for (j, row) in cell.chunks_exact(3).enumerate() {
            out[j * ow + 3 * x..j * ow + 3 * x + 3].copy_from_slice(row);
        }
    }
}

/// Luma and chroma, scaled by 256 (BT.601 weights).
fn yuv([r, g, b, _]: Px) -> [i32; 3] {
    let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
    [
        77 * r + 150 * g + 29 * b,
        -43 * r - 85 * g + 128 * b,
        128 * r - 107 * g - 21 * b,
    ]
}

/// The xBR colour distance.
fn dist(a: [i32; 3], b: [i32; 3]) -> u32 {
    48 * a[0].abs_diff(b[0]) + 7 * a[1].abs_diff(b[1]) + 6 * a[2].abs_diff(b[2])
}

/// `px` over `under` with weight `alpha` / 256.
fn blend(under: Px, px: Px, alpha: u16) -> Px {
    let a = u32::from(alpha);
    let mix = |u: u8, p: u8| ((u32::from(p) * a + u32::from(u) * (256 - a) + 128) >> 8) as u8;
    [
        mix(under[0], px[0]),
        mix(under[1], px[1]),
        mix(under[2], px[2]),
        under[3],
    ]
}

/// Kinds of corner edge, indexing [`Coverage`].
const DIAGONAL: usize = 0;
const SHALLOW: usize = 1;
const STEEP: usize = 2;
const BOTH: usize = 3;

/// Blend weights (of 256) per edge kind and sub-pixel `j * k + i` of the bottom-right corner,
/// for one factor k.
type Coverage = [[u16; 16]; 4];

/// The coverage tables for k = 2, 3, 4.
fn coverage_tables() -> &'static [Coverage; 3] {
    static TABLES: OnceLock<[Coverage; 3]> = OnceLock::new();
    TABLES.get_or_init(|| [coverage(2), coverage(3), coverage(4)])
}

/// Exact sub-pixel coverage of the three edge lines (module docs) for factor `k`.
fn coverage(k: usize) -> Coverage {
    // Half-planes a·x + b·y ≥ c beyond each line, in the unit pixel with the corner at (1, 1).
    let planes = [(1.0, 1.0, 1.5), (1.0, 2.0, 2.0), (2.0, 1.0, 2.0)];
    let mut t = [[0u16; 16]; 4];
    let s = 1.0 / k as f64;
    for j in 0..k {
        for i in 0..k {
            let square = [
                (i as f64 * s, j as f64 * s),
                ((i + 1) as f64 * s, j as f64 * s),
                ((i + 1) as f64 * s, (j + 1) as f64 * s),
                (i as f64 * s, (j + 1) as f64 * s),
            ];
            for (kind, &plane) in planes.iter().enumerate() {
                let frac = clipped_area(&square, plane) / (s * s);
                t[kind][j * k + i] = (frac * 256.0).round() as u16;
            }
            t[BOTH][j * k + i] = t[SHALLOW][j * k + i].max(t[STEEP][j * k + i]);
        }
    }
    t
}

/// Area of the convex polygon `poly` where a·x + b·y ≥ c (Sutherland–Hodgman, then shoelace).
fn clipped_area(poly: &[(f64, f64)], (a, b, c): (f64, f64, f64)) -> f64 {
    let side = |(x, y): (f64, f64)| a * x + b * y - c;
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(poly.len() + 1);
    for (n, &p) in poly.iter().enumerate() {
        let q = poly[(n + 1) % poly.len()];
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            let t = sp / (sp - sq);
            out.push((p.0 + t * (q.0 - p.0), p.1 + t * (q.1 - p.1)));
        }
    }
    let twice: f64 = (0..out.len())
        .map(|n| {
            let (p, q) = (out[n], out[(n + 1) % out.len()]);
            p.0 * q.1 - q.0 * p.1
        })
        .sum();
    twice.abs() / 2.0
}

/// xBR level 2 for input row `y` into its k output rows.
fn xbr_row(p: &Padded, img: &Src<'_>, k: usize, table: &Coverage, y: usize, out: &mut [Px]) {
    let row = p.row(y);
    let pw = p.pw;
    let ow = img.w * k;
    for x in 0..img.w {
        let ie = row + x;
        let e = p.px[ie];
        let mut cell = [e; 16];
        // A corner needs E to differ from both of its side neighbours.
        let flat = [ie - 1, ie + 1, ie - pw, ie + pw]
            .iter()
            .all(|&i| p.px[i] == e);
        if !flat {
            corner::<1, 1>(p, ie, k, table, &mut cell);
            corner::<-1, 1>(p, ie, k, table, &mut cell);
            corner::<1, -1>(p, ie, k, table, &mut cell);
            corner::<-1, -1>(p, ie, k, table, &mut cell);
        }
        for j in 0..k {
            out[j * ow + k * x..j * ow + k * x + k].copy_from_slice(&cell[j * k..j * k + k]);
        }
    }
}

/// The edge rule for the corner of the pixel at padded index `ie` in direction (SX, SY): the
/// rule is written for the bottom-right corner, and (u, v) offsets are mirrored for the
/// others. Blends into `cell` (k × k sub-pixels).
#[inline(always)]
fn corner<const SX: isize, const SY: isize>(
    p: &Padded,
    ie: usize,
    k: usize,
    table: &Coverage,
    cell: &mut [Px; 16],
) {
    let pw = p.pw as isize;
    let n = |u: isize, v: isize| (ie as isize + SX * u + SY * v * pw) as usize;
    let d =
        |a: (isize, isize), b: (isize, isize)| p.d(ie, (SX * a.0, SY * a.1), (SX * b.0, SY * b.1));
    let (e, f, h) = (p.px[ie], p.px[n(1, 0)], p.px[n(0, 1)]);
    if e == f || e == h {
        return;
    }
    // With E (0, 0), F (1, 0), H (0, 1), I (1, 1), C (1, −1), G (−1, 1), D (−1, 0), B (0, −1),
    // F4 (2, 0), I4 (2, 1), H5 (0, 2), I5 (1, 2): distances between pairs parallel to the
    // corner's anti-diagonal (H–F) and to its diagonal (E–I). An edge runs along the
    // anti-diagonal, cutting E's corner, when the colours change less along it than across it.
    let along_hf = d((0, 0), (1, -1))
        + d((0, 0), (-1, 1))
        + d((1, 1), (0, 2))
        + d((1, 1), (2, 0))
        + 4 * d((0, 1), (1, 0));
    let along_ei = d((0, 1), (-1, 0))
        + d((0, 1), (1, 2))
        + d((1, 0), (2, 1))
        + d((1, 0), (0, -1))
        + 4 * d((0, 0), (1, 1));
    if along_hf >= along_ei {
        return;
    }
    let px = if d((0, 0), (1, 0)) <= d((0, 0), (0, 1)) {
        f
    } else {
        h
    };
    let (ke, ki) = (d((1, 0), (-1, 1)), d((0, 1), (1, -1)));
    let (g, c) = (p.px[n(-1, 1)], p.px[n(1, -1)]);
    let shallow = 2 * ke <= ki && e != g && p.px[n(-1, 0)] != g;
    let steep = 2 * ki <= ke && e != c && p.px[n(0, -1)] != c;
    let kind = match (shallow, steep) {
        (true, true) => BOTH,
        (true, false) => SHALLOW,
        (false, true) => STEEP,
        (false, false) => DIAGONAL,
    };
    for j in 0..k {
        for i in 0..k {
            let a = table[kind][j * k + i];
            if a == 0 {
                continue;
            }
            let ox = if SX > 0 { i } else { k - 1 - i };
            let oy = if SY > 0 { j } else { k - 1 - j };
            cell[oy * k + ox] = blend(cell[oy * k + ox], px, a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: Px = [10, 20, 30, 255];
    const FG: Px = [250, 200, 40, 255];

    fn up(src: &[Px], w: usize, h: usize, split: Option<usize>, kind: Upscaler) -> Vec<Px> {
        let mut dst = Vec::new();
        upscale(src, (w, h), split, kind, &mut dst, &mut Scratch::default());
        dst
    }

    const FILTERS: [Upscaler; 6] = [
        Upscaler::Scale2x,
        Upscaler::Scale3x,
        Upscaler::Scale4x,
        Upscaler::Xbr2,
        Upscaler::Xbr3,
        Upscaler::Xbr4,
    ];

    #[test]
    fn a_uniform_image_stays_uniform() {
        let src = vec![FG; 13 * 7];
        for kind in FILTERS {
            let k = kind.factor();
            let out = up(&src, 13, 7, Some(4), kind);
            assert_eq!(out.len(), 13 * k * 7 * k, "{kind:?}");
            assert!(out.iter().all(|&p| p == FG), "{kind:?}");
        }
    }

    #[test]
    fn scale2x_rounds_a_diagonal_line_like_epx() {
        // A one-pixel diagonal line on an 8 × 8 image; check the interior pixels.
        let n = 8;
        let mut src = vec![BG; n * n];
        for i in 0..n {
            src[i * n + i] = FG;
        }
        let out = up(&src, n, n, None, Upscaler::Scale2x);
        let block = |x: usize, y: usize| {
            let o = |dx, dy| out[(2 * y + dy) * 2 * n + 2 * x + dx];
            [o(0, 0), o(1, 0), o(0, 1), o(1, 1)]
        };
        for i in 2..n - 2 {
            assert_eq!(block(i, i), [FG; 4], "line pixels stay whole");
            assert_eq!(
                block(i + 1, i),
                [BG, BG, FG, BG],
                "right of the line: E2 filled"
            );
            assert_eq!(
                block(i, i + 1),
                [BG, FG, BG, BG],
                "below the line: E1 filled"
            );
            assert_eq!(block(i + 2, i), [BG; 4]);
            assert_eq!(block(i, i + 2), [BG; 4]);
        }
    }

    #[test]
    fn scale3x_and_scale4x_keep_single_pixels_and_fill_diagonals() {
        let mut dot = vec![BG; 25];
        dot[12] = FG;
        let out = up(&dot, 5, 5, None, Upscaler::Scale3x);
        assert_eq!(
            out.iter().filter(|&&p| p == FG).count(),
            9,
            "isolated pixel"
        );
        // Scale2x keeps it a 2 × 2 block; the second pass rounds the block's four corners.
        let out = up(&dot, 5, 5, None, Upscaler::Scale4x);
        assert_eq!(
            out.iter().filter(|&&p| p == FG).count(),
            12,
            "isolated pixel"
        );
        // A diagonal pair gains pixels between them.
        let mut pair = vec![BG; 25];
        pair[6] = FG;
        pair[12] = FG;
        let out = up(&pair, 5, 5, None, Upscaler::Scale3x);
        assert!(out.iter().filter(|&&p| p == FG).count() > 18);
    }

    /// Filtering with a split equals filtering both bands separately.
    #[test]
    fn the_split_row_is_a_hard_edge() {
        let (w, h, s) = (9, 8, 5);
        let src: Vec<Px> = (0..w * h)
            .map(|i| if (i % w + i / w) % 3 == 0 { FG } else { BG })
            .collect();
        for kind in FILTERS {
            let k = kind.factor();
            let whole = up(&src, w, h, Some(s), kind);
            let top = up(&src[..w * s], w, s, None, kind);
            let bottom = up(&src[w * s..], w, h - s, None, kind);
            assert_eq!(&whole[..top.len()], top.as_slice(), "{kind:?} top");
            assert_eq!(&whole[top.len()..], bottom.as_slice(), "{kind:?} bottom");
            assert_eq!(whole.len(), w * k * h * k);
        }
    }

    #[test]
    fn coverage_gives_the_published_weights() {
        let [t2, t3, t4] = coverage_tables();
        // 2×: corner sub-pixel 3 (bottom right); 2 is bottom left, 1 top right.
        assert_eq!(t2[DIAGONAL][..4], [0, 0, 0, 128]);
        assert_eq!(t2[SHALLOW][..4], [0, 0, 64, 192]);
        assert_eq!(t2[STEEP][..4], [0, 64, 0, 192]);
        // 3×.
        assert_eq!(t3[DIAGONAL][..9], [0, 0, 0, 0, 0, 32, 0, 32, 224]);
        assert_eq!(t3[SHALLOW][..9], [0, 0, 0, 0, 0, 64, 64, 192, 256]);
        assert_eq!(t3[STEEP][..9], [0, 0, 64, 0, 0, 192, 0, 64, 256]);
        // 4×: the diagonal covers the corner and half of its two neighbours.
        assert_eq!(t4[DIAGONAL][15], 256);
        assert_eq!(
            (t4[DIAGONAL][14], t4[DIAGONAL][11], t4[DIAGONAL][10]),
            (128, 128, 0)
        );
    }

    /// A small synthetic sprite: a filled diamond with an outline, a shallow and a steep line.
    fn sprite() -> (Vec<Px>, usize, usize) {
        let (w, h) = (16, 12);
        let ink = [40, 220, 90, 255];
        let mut px = vec![BG; w * h];
        for y in 0..h {
            for x in 0..w {
                let (dx, dy) = ((x as i32 - 5).abs(), (y as i32 - 5).abs());
                if dx + dy <= 3 {
                    px[y * w + x] = if dx + dy == 3 { FG } else { ink };
                }
            }
        }
        for x in 8..16 {
            px[(11 - (x - 8) / 2) * w + x] = FG; // shallow
        }
        for y in 0..8 {
            px[y * w + 12 + y / 4] = ink; // steep
        }
        (px, w, h)
    }

    fn hash(px: &[Px]) -> String {
        td_test_support::sha256_hex(px.as_flattened())
    }

    #[test]
    fn xbr_golden_hashes() {
        let (src, w, h) = sprite();
        let got: Vec<String> = [Upscaler::Xbr2, Upscaler::Xbr3, Upscaler::Xbr4]
            .into_iter()
            .map(|kind| hash(&up(&src, w, h, Some(8), kind)))
            .collect();
        assert_eq!(got, XBR_GOLDEN, "xBR output changed");
    }

    const XBR_GOLDEN: [&str; 3] = [
        "25ef7d0960a5b6d73eb162cdc97c4223f5bc8f6ace9bdcf445a9a0b334269f57",
        "00636d0ed3f87d3ae283d6a353a56e4b2613051e6a1ec9578fe6b3bd0e8a9a00",
        "8705db04f8b8a49ae3b773eed63847268e6c0a8052a00b877657790246f288d6",
    ];

    #[test]
    fn xbr_smooths_a_diagonal_edge() {
        // Below the main diagonal FG, above BG: the corner sub-pixels along the edge blend.
        let n = 8;
        let src: Vec<Px> = (0..n * n)
            .map(|i| if i % n <= i / n { FG } else { BG })
            .collect();
        let out = up(&src, n, n, None, Upscaler::Xbr2);
        let mixed = out.iter().filter(|&&p| p != FG && p != BG).count();
        assert!(mixed >= n, "blended sub-pixels along the edge: {mixed}");
        // Far from the edge nothing changes.
        assert_eq!(out[2 * n * 2 * (n - 1)], FG);
        assert_eq!(out[2 * n - 1], BG);
    }
}
