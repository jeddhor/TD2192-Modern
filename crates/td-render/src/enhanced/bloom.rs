//! Bloom (M12-12): the glow of emissive and bright pixels, added at output resolution after
//! the upscale and the light map.
//!
//! Spec: `docs/spec/enhanced/presentation.md` rev 3 §6.3 (design, T3, Enhanced profile only;
//! ADR-0006). Steps: bright pass (emissive colours and pixels whose linear luminance is above
//! 0.9), 4 × 4 box downsample, a separable 9-tap Gaussian (σ = 2 quarter-resolution pixels,
//! horizontal then vertical), bilinear upsample, added in linear light at strength 0.6.
//! Deterministic: integer arithmetic at full resolution, plain IEEE `+ − × ÷` with fixed
//! weights at quarter resolution, and the row split changes no result.
//!
//! Cost (M12-18, spec rev 6 §7): the bright pass of the lit rows runs inside the light map's
//! modulation ([`super::lighting::LightMap::apply_with_bloom`]) while the pixels are at hand;
//! the blur and the upsample visit only the columns between the first and the last that hold
//! any glow in each row. The output is bit-identical to computing every pixel.

use rayon::prelude::*;

use super::lighting::{FRAME_ROWS, LIN_MAX, Srgb, Tap, srgb, tap};
use super::scene::Emissive;

/// Linear luminance above which a pixel blooms.
pub const THRESHOLD: f32 = 0.9;
/// Strength of the added glow.
pub const STRENGTH: f32 = 0.6;
/// Downsample factor per axis.
pub const DOWNSAMPLE: usize = 4;
/// The 9-tap Gaussian, σ = 2, normalised: weights for offsets 0, ±1, ..., ±4.
pub const GAUSS: [f32; 5] = [
    0.204_163_69,
    0.180_173_83,
    0.123_831_54,
    0.066_282_24,
    0.027_630_55,
];
/// Rec. 709 luminance weights (linear light) in 1/65536.
const LUMA: [u32; 3] = [13_933, 46_871, 4_732];
/// [`THRESHOLD`] in the units of a weighted sum: 0.9 · 16383 · 65536.
const THRESHOLD_SUM: u32 = 966_308_659;

/// The bright-pass test of one output pixel, with the tables it needs.
#[derive(Clone, Copy)]
pub(crate) struct BrightTest {
    t: &'static Srgb,
    /// The darkest green byte whose pixel can pass the threshold whatever its red and blue: a
    /// pixel with less green has a luminance of at most `LUMA[0] + lin(g) LUMA[1] + LUMA[2]`,
    /// which is not above it. An exact shortcut for the many dark pixels.
    g_min: u8,
}

impl BrightTest {
    pub(crate) fn new() -> Self {
        let t = srgb();
        let most = LIN_MAX * LUMA[0] + LIN_MAX * LUMA[2];
        let g_min = (0..=255u8)
            .find(|&g| most + u32::from(t.lin[usize::from(g)]) * LUMA[1] > THRESHOLD_SUM)
            .unwrap_or(255);
        Self { t, g_min }
    }

    /// Adds the pixel `px` (RGBA) to `sum` when it passes: always when `emissive`, else when its
    /// linear luminance is above [`THRESHOLD`].
    #[inline]
    pub(crate) fn add(&self, px: &[u8], emissive: bool, sum: &mut [u32; 3]) {
        if !emissive && px[1] < self.g_min {
            return;
        }
        let l = [0, 1, 2].map(|c| u32::from(self.t.lin[usize::from(px[c])]));
        if emissive || l[0] * LUMA[0] + l[1] * LUMA[1] + l[2] * LUMA[2] > THRESHOLD_SUM {
            for c in 0..3 {
                sum[c] += l[c];
            }
        }
    }

    /// The bright pass of output row `y` (`row`, `ow` RGBA pixels) of a frame upscaled `k`
    /// times from `w` base pixels: adds each passing pixel to its quarter cell of `sum`.
    pub(crate) fn add_row(
        &self,
        row: &[u8],
        y: usize,
        (w, k): (usize, usize),
        indexed: &[u8],
        emissive: &Emissive,
        sum: &mut [[u32; 3]],
    ) {
        let src = &indexed[(y / k) * w..][..w];
        for (bx, (block, &c)) in row.chunks_exact(4 * k).zip(src).enumerate() {
            let em = emissive.0[usize::from(c)];
            for (dx, px) in block.chunks_exact(4).enumerate() {
                self.add(px, em, &mut sum[(bx * k + dx) / DOWNSAMPLE]);
            }
        }
    }
}

/// The box means of one quarter-resolution row from its bright-pass sums: `bh` output rows
/// high, `ow` output pixels wide (linear light, 0..=1).
pub(crate) fn box_means(sum: &[[u32; 3]], down: &mut [[f32; 3]], ow: usize, bh: usize) {
    for (qx, (s, d)) in sum.iter().zip(down.iter_mut()).enumerate() {
        let bw = (ow - qx * DOWNSAMPLE).min(DOWNSAMPLE);
        let scale = 1.0 / ((bw * bh) as f32 * LIN_MAX as f32);
        *d = s.map(|v| v as f32 * scale);
    }
}

/// The bloom stage and its scratch buffers, reused from frame to frame.
#[derive(Debug, Default)]
pub struct Bloom {
    qw: usize,
    qh: usize,
    /// Bright-pass sums per quarter-resolution pixel (14-bit linear units).
    sum: Vec<[u32; 3]>,
    down: Vec<[f32; 3]>,
    tmp: Vec<[f32; 3]>,
    /// The blurred glow × strength, in 14-bit linear units.
    glow: Vec<[u32; 3]>,
    /// The first and last column of each quarter-resolution row holding any glow.
    glowing: Vec<Option<(usize, usize)>>,
    /// The first and last non-zero column of each row of the horizontal pass.
    live: Vec<Option<(usize, usize)>>,
    cols: Vec<Tap>,
    rows: Vec<Tap>,
    /// Per quarter column q: the first output column whose tap reads q or a later column, and
    /// the last whose tap reads q or an earlier one.
    reach: Vec<(usize, usize)>,
    taps_for: (usize, usize),
}

/// Index `i` clamped to `0..n` (edges repeat).
fn clamp(i: isize, n: usize) -> usize {
    i.clamp(0, n as isize - 1) as usize
}

/// The first and last index of `row` that is not all zero.
fn extent<T: Copy + PartialEq + Default>(row: &[[T; 3]]) -> Option<(usize, usize)> {
    let lit = |v: &[T; 3]| *v != [T::default(); 3];
    let first = row.iter().position(lit)?;
    let last = row.iter().rposition(lit)?;
    Some((first, last))
}

/// The union of two column ranges.
fn union(a: Option<(usize, usize)>, b: Option<(usize, usize)>) -> Option<(usize, usize)> {
    match (a, b) {
        (Some((a0, a1)), Some((b0, b1))) => Some((a0.min(b0), a1.max(b1))),
        (a, None) => a,
        (None, b) => b,
    }
}

impl Bloom {
    fn ensure_taps(&mut self, ow: usize, oh: usize) {
        if self.taps_for == (ow, oh) {
            return;
        }
        self.taps_for = (ow, oh);
        self.cols.clear();
        self.cols
            .extend((0..ow).map(|x| tap(x, DOWNSAMPLE, self.qw)));
        self.rows.clear();
        self.rows
            .extend((0..oh).map(|y| tap(y, DOWNSAMPLE, self.qh)));
        // Taps never move left as x grows, so these are the bounds of the output columns that
        // can see column q.
        self.reach.clear();
        self.reach.extend((0..self.qw).map(|q| {
            let first = self
                .cols
                .iter()
                .position(|t| usize::from(t.i1) >= q)
                .unwrap_or(ow);
            let last = self
                .cols
                .iter()
                .rposition(|t| usize::from(t.i0) <= q)
                .unwrap_or(0);
            (first, last)
        }));
    }

    /// Sizes the buffers for a (w·k) × (200·k) output and returns the quarter-resolution
    /// sums and means, for a bright pass done elsewhere
    /// ([`super::lighting::LightMap::apply_with_bloom`]).
    pub(crate) fn begin(&mut self, w: usize, k: usize) -> (&mut [[u32; 3]], &mut [[f32; 3]]) {
        assert!(w >= 1 && k >= 1, "size");
        let (ow, oh) = (w * k, FRAME_ROWS * k);
        let (qw, qh) = (ow.div_ceil(DOWNSAMPLE), oh.div_ceil(DOWNSAMPLE));
        self.qw = qw;
        self.qh = qh;
        self.ensure_taps(ow, oh);
        let n = qw * qh;
        self.sum.resize(n, [0; 3]);
        self.down.resize(n, [0.0; 3]);
        self.tmp.resize(n, [0.0; 3]);
        self.glow.resize(n, [0; 3]);
        self.glowing.resize(qh, None);
        self.live.resize(qh, None);
        (&mut self.sum, &mut self.down)
    }

    /// Adds bloom to the finished frame in place: `rgba` is (w·k) × (200·k) RGBA8 and
    /// `indexed` the composed w × 200 frame (for the emissive colours). Alpha is untouched.
    ///
    /// # Panics
    /// If the buffers do not match `w` and `k`.
    pub fn apply(
        &mut self,
        rgba: &mut [u8],
        w: usize,
        k: usize,
        indexed: &[u8],
        emissive: &Emissive,
    ) {
        self.begin(w, k);
        self.finish(rgba, w, k, indexed, emissive, 0);
    }

    /// The bright pass of quarter rows `from..` (the earlier ones were done with the light
    /// map), then the blur and the glow added to `rgba`. [`Self::begin`] came first.
    ///
    /// # Panics
    /// If the buffers do not match `w` and `k`.
    pub(crate) fn finish(
        &mut self,
        rgba: &mut [u8],
        w: usize,
        k: usize,
        indexed: &[u8],
        emissive: &Emissive,
        from: usize,
    ) {
        let (ow, oh) = (w * k, FRAME_ROWS * k);
        assert_eq!(rgba.len(), ow * oh * 4, "RGBA size");
        assert_eq!(indexed.len(), w * FRAME_ROWS, "indexed size");
        let (qw, qh) = (self.qw, self.qh);
        assert_eq!((qw, qh), (ow.div_ceil(DOWNSAMPLE), oh.div_ceil(DOWNSAMPLE)));
        let t = srgb();
        let test = BrightTest::new();

        // 1. Bright pass and box downsample: the mean of each 4 × 4 block, counting only the
        // pixels that pass (linear light, 0..=1).
        let frame: &[u8] = rgba;
        self.sum[from * qw..]
            .par_chunks_mut(qw)
            .zip(self.down[from * qw..].par_chunks_mut(qw))
            .enumerate()
            .for_each(|(i, (sum, down))| {
                sum.fill([0; 3]);
                let y0 = (from + i) * DOWNSAMPLE;
                let y1 = (y0 + DOWNSAMPLE).min(oh);
                for y in y0..y1 {
                    let row = &frame[y * ow * 4..][..ow * 4];
                    test.add_row(row, y, (w, k), indexed, emissive, sum);
                }
                box_means(sum, down, ow, y1 - y0);
            });

        // 2. Blur: horizontal into `tmp`, then vertical back into `down`; edges repeat.
        // Most of the image is black: each non-zero pixel is spread to its nine neighbours,
        // which adds the same terms in the same order as the gather (zero terms add nothing).
        self.down
            .par_chunks(qw)
            .zip(self.tmp.par_chunks_mut(qw))
            .zip(self.live.par_iter_mut())
            .for_each(|((src, dst), live)| {
                dst.fill([0.0; 3]);
                if extent(src).is_none() {
                    *live = None;
                    return;
                }
                for i in -4..qw as isize + 4 {
                    let s = src[clamp(i, qw)];
                    if s == [0.0; 3] {
                        continue;
                    }
                    for o in -4isize..=4 {
                        let Some(d) = usize::try_from(i - o).ok().and_then(|x| dst.get_mut(x))
                        else {
                            continue;
                        };
                        let g = GAUSS[o.unsigned_abs()];
                        for c in 0..3 {
                            d[c] += s[c] * g;
                        }
                    }
                }
                *live = extent(dst);
            });
        let (tmp, live) = (&self.tmp, &self.live);
        let scale = STRENGTH * LIN_MAX as f32;
        self.down
            .par_chunks_mut(qw)
            .zip(self.glow.par_chunks_mut(qw))
            .zip(self.glowing.par_iter_mut())
            .enumerate()
            .for_each(|(y, ((down, glow), glowing))| {
                down.fill([0.0; 3]);
                glow.fill([0; 3]);
                // Only columns some contributing row holds can become non-zero.
                let span =
                    (-4isize..=4).fold(None, |u, o| union(u, live[clamp(y as isize + o, qh)]));
                let Some((c0, c1)) = span else {
                    *glowing = None;
                    return;
                };
                for o in -4isize..=4 {
                    let j = clamp(y as isize + o, qh);
                    if live[j].is_none() {
                        continue;
                    }
                    let src = &tmp[j * qw..][c0..=c1];
                    let g = GAUSS[o.unsigned_abs()];
                    for (d, s) in down[c0..=c1].iter_mut().zip(src) {
                        for c in 0..3 {
                            d[c] += s[c] * g;
                        }
                    }
                }
                for (g, d) in glow[c0..=c1].iter_mut().zip(&down[c0..=c1]) {
                    *g = d.map(|v| (v * scale + 0.5) as u32);
                }
                *glowing = extent(&glow[c0..=c1]).map(|(a, b)| (c0 + a, c0 + b));
            });

        // 3. Bilinear upsample, added in linear light, over the output columns that can see a
        // glowing column of either source row (the others would add nothing).
        let (glow, glowing, cols, rows) = (&self.glow, &self.glowing, &self.cols, &self.rows);
        let reach = &self.reach;
        rgba.par_chunks_exact_mut(ow * 4)
            .enumerate()
            .for_each(|(y, row)| {
                let rt = rows[y];
                let (j0, j1) = (usize::from(rt.i0), usize::from(rt.i1));
                let Some((lo, hi)) = union(glowing[j0], glowing[j1]) else {
                    return;
                };
                let (x0, x1) = (reach[lo].0, reach[hi].1);
                if x0 > x1 {
                    return;
                }
                let (r0, r1) = (&glow[j0 * qw..][..qw], &glow[j1 * qw..][..qw]);
                let (a, b) = (256 - u32::from(rt.f), u32::from(rt.f));
                let column = |i: u16| -> [u32; 3] {
                    let (p, q) = (r0[usize::from(i)], r1[usize::from(i)]);
                    std::array::from_fn(|c| (p[c] * a + q[c] * b) >> 8)
                };
                let mut at = None;
                let (mut v0, mut v1, mut dark) = ([0; 3], [0; 3], true);
                let span = row[x0 * 4..=x1 * 4 + 3].chunks_exact_mut(4);
                for (px, ct) in span.zip(&cols[x0..=x1]) {
                    if at != Some((ct.i0, ct.i1)) {
                        at = Some((ct.i0, ct.i1));
                        (v0, v1) = (column(ct.i0), column(ct.i1));
                        dark = v0 == [0; 3] && v1 == [0; 3];
                    }
                    if dark {
                        continue;
                    }
                    let (a, b) = (256 - u32::from(ct.f), u32::from(ct.f));
                    let g: [u32; 3] = std::array::from_fn(|c| (v0[c] * a + v1[c] * b) >> 8);
                    if g == [0; 3] {
                        continue;
                    }
                    for c in 0..3 {
                        let l = u32::from(t.lin[usize::from(px[c])]) + g[c];
                        px[c] = t.enc[l.min(LIN_MAX) as usize];
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn gaussian_is_normalised() {
        let sum = GAUSS[0] + 2.0 * GAUSS[1..].iter().sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-6, "{sum}");
        assert!(GAUSS.windows(2).all(|p| p[0] > p[1]));
        let luma = LUMA.iter().sum::<u32>();
        assert_eq!(luma, 65536);
        let want = (THRESHOLD as f64 * f64::from(LIN_MAX) * 65536.0) as u32;
        assert!(THRESHOLD_SUM.abs_diff(want) <= 64, "{want}");
    }

    /// The green shortcut rejects only pixels the full test rejects (M12-18).
    #[test]
    fn the_green_shortcut_is_exact() {
        let test = BrightTest::new();
        let t = srgb();
        assert!(test.g_min > 200, "{}", test.g_min);
        for g in 0..test.g_min {
            let l = [255u8, g, 255].map(|c| u32::from(t.lin[usize::from(c)]));
            assert!(l[0] * LUMA[0] + l[1] * LUMA[1] + l[2] * LUMA[2] <= THRESHOLD_SUM);
        }
        let mut sum = [0; 3];
        test.add(&[255, test.g_min - 1, 255, 255], false, &mut sum);
        assert_eq!(sum, [0; 3]);
        test.add(&[10, 10, 10, 255], true, &mut sum);
        assert_ne!(sum, [0; 3], "emissive always passes");
    }

    fn frame(w: usize, k: usize, fill: [u8; 3]) -> Vec<u8> {
        [fill[0], fill[1], fill[2], 255].repeat(w * k * FRAME_ROWS * k)
    }

    #[test]
    fn dark_frame_without_emissive_is_unchanged() {
        let (w, k) = (320, 2);
        let rgba = frame(w, k, [120, 90, 200]);
        let mut out = rgba.clone();
        Bloom::default().apply(&mut out, w, k, &vec![1; w * 200], &Emissive::default());
        assert_eq!(out, rgba);
    }

    #[test]
    fn an_emissive_dot_glows_symmetrically() {
        let (w, k) = (320, 1);
        let mut indexed = vec![0u8; w * 200];
        let mut rgba = frame(w, k, [10, 10, 10]);
        // A 4 × 4 emissive block on a quarter cell (x 160..164, y 80..84).
        for y in 80..84 {
            for x in 160..164 {
                indexed[y * w + x] = 7;
                rgba[(y * w + x) * 4..][..3].copy_from_slice(&[255, 200, 100]);
            }
        }
        let mut em = Emissive::default();
        em.0[7] = true;
        Bloom::default().apply(&mut rgba, w, k, &indexed, &em);
        let px = |x: usize, y: usize| rgba[(y * w + x) * 4];
        assert!(
            px(170, 82) > 10 && px(153, 82) > 10,
            "glow around the block"
        );
        // The block's centre is (162, 82): mirror pairs match (to rounding).
        for d in 3..14 {
            assert!(px(161 - d, 81).abs_diff(px(162 + d, 81)) <= 1, "x {d}");
            assert!(px(161, 81 - d).abs_diff(px(161, 82 + d)) <= 1, "y {d}");
        }
        assert_eq!(px(10, 10), 10, "far away nothing changes");
        assert!(rgba.chunks(4).all(|p| p[3] == 255), "alpha");
    }

    /// Glow at both edges of the frame (the column ranges reach the edge taps, M12-18).
    #[test]
    fn glow_at_the_frame_edges() {
        let (w, k) = (320, 3);
        let mut indexed = vec![0u8; w * 200];
        let mut rgba = frame(w, k, [10, 10, 10]);
        for (x, y) in [(0, 0), (319, 199), (0, 199), (319, 0), (160, 100)] {
            indexed[y * w + x] = 7;
        }
        let mut em = Emissive::default();
        em.0[7] = true;
        Bloom::default().apply(&mut rgba, w, k, &indexed, &em);
        let (ow, oh) = (w * k, 200 * k);
        let px = |x: usize, y: usize| rgba[(y * ow + x) * 4];
        for (x, y) in [(0, 0), (ow - 1, oh - 1), (0, oh - 1), (ow - 1, 0)] {
            assert!(px(x, y) > 10, "({x}, {y})");
        }
    }

    #[test]
    fn golden_synthetic_frame() {
        let (w, k) = (428, 2);
        let mut indexed = vec![0u8; w * 200];
        let mut rgba = frame(w, k, [30, 40, 60]);
        for (i, (x, y)) in [(20, 30), (200, 100), (420, 150), (100, 175), (0, 0)]
            .into_iter()
            .enumerate()
        {
            indexed[y * w + x] = 9;
            for dy in 0..k {
                for dx in 0..k {
                    let o = ((y * k + dy) * w * k + x * k + dx) * 4;
                    rgba[o..o + 3].copy_from_slice(&[250, 120 + 20 * i as u8, 60]);
                }
            }
        }
        // A bright non-emissive patch.
        for y in 60..70 {
            for x in 300..310 {
                let o = (y * w * k + x) * 4;
                rgba[o..o + 3].copy_from_slice(&[255, 255, 250]);
            }
        }
        let mut em = Emissive::default();
        em.0[9] = true;
        let mut bloom = Bloom::default();
        bloom.apply(&mut rgba, w, k, &indexed, &em);
        let digest: String = Sha256::digest(&rgba)
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(digest, "e4d69368294d9212");
    }
}
