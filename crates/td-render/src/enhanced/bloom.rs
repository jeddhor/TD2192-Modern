//! Bloom (M12-12): the glow of emissive and bright pixels, added at output resolution after
//! the upscale and the light map.
//!
//! Spec: `docs/spec/enhanced/presentation.md` rev 2 §6.3 (design, T3, Enhanced profile only;
//! ADR-0006). Steps: bright pass (emissive colours and pixels whose linear luminance is above
//! 0.9), 4 × 4 box downsample, a separable 9-tap Gaussian (σ = 2 quarter-resolution pixels,
//! horizontal then vertical), bilinear upsample, added in linear light at strength 0.6.
//! Deterministic: integer arithmetic at full resolution, plain IEEE `+ − × ÷` with fixed
//! weights at quarter resolution, and the row split changes no result.

use rayon::prelude::*;

use super::lighting::{FRAME_ROWS, LIN_MAX, Tap, srgb, tap};
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
    /// Quarter-resolution rows holding any glow.
    glowing: Vec<bool>,
    cols: Vec<Tap>,
    rows: Vec<Tap>,
    taps_for: (usize, usize),
}

/// Index `i` clamped to `0..n` (edges repeat).
fn clamp(i: isize, n: usize) -> usize {
    i.clamp(0, n as isize - 1) as usize
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
        assert!(w >= 1 && k >= 1, "size");
        let (ow, oh) = (w * k, FRAME_ROWS * k);
        assert_eq!(rgba.len(), ow * oh * 4, "RGBA size");
        assert_eq!(indexed.len(), w * FRAME_ROWS, "indexed size");
        let (qw, qh) = (ow.div_ceil(DOWNSAMPLE), oh.div_ceil(DOWNSAMPLE));
        self.qw = qw;
        self.qh = qh;
        self.ensure_taps(ow, oh);
        let t = srgb();
        let n = qw * qh;
        self.sum.resize(n, [0; 3]);
        self.down.resize(n, [0.0; 3]);
        self.tmp.resize(n, [0.0; 3]);
        self.glow.resize(n, [0; 3]);
        self.glowing.resize(qh, false);

        // 1. Bright pass and box downsample: the mean of each 4 × 4 block, counting only the
        // pixels that pass (linear light, 0..=1).
        let frame: &[u8] = rgba;
        // The darkest byte whose linear value can pass the threshold.
        let bright_min = (0..=255u8)
            .find(|&c| u32::from(t.lin[usize::from(c)]) * 65536 > THRESHOLD_SUM)
            .unwrap_or(255);
        self.sum
            .par_chunks_mut(qw)
            .zip(self.down.par_chunks_mut(qw))
            .enumerate()
            .for_each(|(qy, (sum, down))| {
                sum.fill([0; 3]);
                let y0 = qy * DOWNSAMPLE;
                let y1 = (y0 + DOWNSAMPLE).min(oh);
                for y in y0..y1 {
                    let row = &frame[y * ow * 4..][..ow * 4];
                    let src = &indexed[(y / k) * w..][..w];
                    for (bx, (block, &c)) in row.chunks_exact(4 * k).zip(src).enumerate() {
                        let em = emissive.0[usize::from(c)];
                        for (dx, px) in block.chunks_exact(4).enumerate() {
                            // Luminance is at most the brightest channel's linear value.
                            if !em && px[0].max(px[1]).max(px[2]) < bright_min {
                                continue;
                            }
                            let l = [0, 1, 2].map(|c| u32::from(t.lin[usize::from(px[c])]));
                            let lum = l[0] * LUMA[0] + l[1] * LUMA[1] + l[2] * LUMA[2];
                            if em || lum > THRESHOLD_SUM {
                                let s = &mut sum[(bx * k + dx) / DOWNSAMPLE];
                                for c in 0..3 {
                                    s[c] += l[c];
                                }
                            }
                        }
                    }
                }
                let bh = y1 - y0;
                for (qx, (s, d)) in sum.iter().zip(down.iter_mut()).enumerate() {
                    let bw = (ow - qx * DOWNSAMPLE).min(DOWNSAMPLE);
                    let scale = 1.0 / ((bw * bh) as f32 * LIN_MAX as f32);
                    *d = s.map(|v| v as f32 * scale);
                }
            });

        // 2. Blur: horizontal into `tmp`, then vertical back into `down`; edges repeat.
        self.down
            .par_chunks(qw)
            .zip(self.tmp.par_chunks_mut(qw))
            .for_each(|(src, dst)| {
                for (x, d) in dst.iter_mut().enumerate() {
                    let mut acc = [0.0f32; 3];
                    for o in -4isize..=4 {
                        let s = src[clamp(x as isize + o, qw)];
                        let g = GAUSS[o.unsigned_abs()];
                        for c in 0..3 {
                            acc[c] += s[c] * g;
                        }
                    }
                    *d = acc;
                }
            });
        let tmp = &self.tmp;
        let scale = STRENGTH * LIN_MAX as f32;
        self.down
            .par_chunks_mut(qw)
            .zip(self.glow.par_chunks_mut(qw))
            .zip(self.glowing.par_iter_mut())
            .enumerate()
            .for_each(|(y, ((down, glow), glowing))| {
                down.fill([0.0; 3]);
                for o in -4isize..=4 {
                    let src = &tmp[clamp(y as isize + o, qh) * qw..][..qw];
                    let g = GAUSS[o.unsigned_abs()];
                    for (d, s) in down.iter_mut().zip(src) {
                        for c in 0..3 {
                            d[c] += s[c] * g;
                        }
                    }
                }
                for (g, d) in glow.iter_mut().zip(down.iter()) {
                    *g = d.map(|v| (v * scale + 0.5) as u32);
                }
                *glowing = glow.iter().any(|g| g.iter().any(|&c| c != 0));
            });

        // 3. Bilinear upsample, added in linear light.
        let (glow, glowing, cols, rows) = (&self.glow, &self.glowing, &self.cols, &self.rows);
        rgba.par_chunks_exact_mut(ow * 4)
            .enumerate()
            .for_each(|(y, row)| {
                let rt = rows[y];
                let (j0, j1) = (usize::from(rt.i0), usize::from(rt.i1));
                if !glowing[j0] && !glowing[j1] {
                    return;
                }
                let (r0, r1) = (&glow[j0 * qw..][..qw], &glow[j1 * qw..][..qw]);
                let (a, b) = (256 - u32::from(rt.f), u32::from(rt.f));
                let column = |i: u16| -> [u32; 3] {
                    let (p, q) = (r0[usize::from(i)], r1[usize::from(i)]);
                    std::array::from_fn(|c| (p[c] * a + q[c] * b) >> 8)
                };
                let mut at = None;
                let (mut v0, mut v1) = ([0; 3], [0; 3]);
                for (px, ct) in row.chunks_exact_mut(4).zip(cols) {
                    if at != Some((ct.i0, ct.i1)) {
                        at = Some((ct.i0, ct.i1));
                        (v0, v1) = (column(ct.i0), column(ct.i1));
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
