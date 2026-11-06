//! Dynamic lighting (M12-11): the light map with soft shadows from solid map cells, emissive
//! colours, and the modulation of the upscaled RGBA image.
//!
//! Spec: `docs/spec/enhanced/presentation.md` rev 2 §6 (design, T3, Enhanced profile only;
//! ADR-0006). Presentation only: the scene is built from what the flight drew, and nothing
//! here feeds back into the simulation. Deterministic: per-frame work uses only IEEE `+ − × ÷`,
//! `sqrt` and `floor` (no fused or transcendental operations), and the row split changes no
//! result.
//!
//! The map holds one gain per channel for every 2 × 2 block of the playfield (rows 0..160),
//! in 12-bit fixed point, sampled bilinearly at output resolution. Pixels are modulated in
//! linear light through 8-bit ↔ 14-bit sRGB tables whose round trip is exact, so a gain of 1
//! leaves a pixel unchanged.

use std::sync::OnceLock;

use rayon::prelude::*;

use super::bloom::Bloom;
use super::scene::{Emissive, Light, LightMode, LightScene, Occluders};
use super::{Config, FrameInput, PostStage};

/// Rows of the flight playfield; the HUD rows below are never lit (§3).
pub const LIT_ROWS: usize = 160;
/// Rows of a composed frame.
pub const FRAME_ROWS: usize = 200;
/// Side of an occluder cell (one map tile) in base pixels.
pub const CELL: i32 = 32;
/// Largest gain per channel: light lifts a pixel to at most 1.6 × its daytime colour (§6.1).
pub const MAX_GAIN: f32 = 1.6;
/// Ambient of `full` ("night", §6.1); `subtle` uses 1.
pub const NIGHT_AMBIENT: [f32; 3] = [0.38, 0.42, 0.55];
/// Shadow march step in base pixels, and the factor per solid cell crossed (§6.2).
pub const SHADOW_STEP: f32 = 4.0;
pub const SHADOW_FACTOR: f32 = 0.25;
/// The march stops once visibility is this low (four cells: 1/256).
const MIN_VISIBILITY: f32 = 1.0 / 256.0;
/// Shot colours, always emissive (§6.1; `flight-screens.md` §4.8).
pub const SHOT_COLOURS: [u8; 6] = [0x3C, 0x3D, 0x32, 0x33, 0x35, 0xBE];
/// Widest frame the light map supports.
pub const MAX_WIDTH: usize = 2048;
/// Half-resolution rows of the map.
const HALF_ROWS: usize = LIT_ROWS / 2;

/// Fixed-point gain: 1.0 = 4096.
pub(crate) const GAIN_BITS: u32 = 12;
pub(crate) const GAIN_ONE: u32 = 1 << GAIN_BITS;
/// Linear light in 14 bits.
pub(crate) const LIN_MAX: u32 = (1 << 14) - 1;

/// sRGB transfer tables.
#[derive(Debug)]
pub(crate) struct Srgb {
    /// Byte → linear, 0..=[`LIN_MAX`].
    pub lin: [u16; 256],
    /// Byte → linear, 0..=1.
    pub lin_f: [f32; 256],
    /// Linear 0..=[`LIN_MAX`] → byte; `enc[lin[c]] == c` for every byte.
    pub enc: Box<[u8]>,
}

fn srgb_decode(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_encode(l: f64) -> f64 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// The shared sRGB tables (built once).
pub(crate) fn srgb() -> &'static Srgb {
    static TABLES: OnceLock<Srgb> = OnceLock::new();
    TABLES.get_or_init(|| {
        let (mut lin, mut lin_f) = ([0u16; 256], [0f32; 256]);
        for c in 0..256 {
            let l = srgb_decode(c as f64 / 255.0);
            lin[c] = (l * f64::from(LIN_MAX)).round() as u16;
            lin_f[c] = l as f32;
        }
        let mut enc: Vec<u8> = (0..=LIN_MAX)
            .map(|v| (srgb_encode(f64::from(v) / f64::from(LIN_MAX)) * 255.0).round() as u8)
            .collect();
        // Make the round trip exact (it is already, up to rounding at a few knees).
        for (c, &l) in lin.iter().enumerate() {
            enc[usize::from(l)] = c as u8;
        }
        Srgb {
            lin,
            lin_f,
            enc: enc.into_boxed_slice(),
        }
    })
}

/// Linear RGB of an 8-bit sRGB colour.
pub fn linear_rgb(rgb: [u8; 3]) -> [f32; 3] {
    let t = srgb();
    rgb.map(|c| t.lin_f[usize::from(c)])
}

/// The ambient of a mode (§6.1).
pub fn ambient(mode: LightMode) -> [f32; 3] {
    match mode {
        LightMode::Subtle => [1.0; 3],
        LightMode::Full => NIGHT_AMBIENT,
    }
}

/// The emissive set of a flight (§6.1): the shot colours, the HUD lamp entries, and every
/// colour the explosion frames (`td.exp`) use. Colour 0 is transparent and never emissive.
pub fn flight_emissive<'a>(explosion_frames: impl IntoIterator<Item = &'a [u8]>) -> Emissive {
    let mut set = [false; 256];
    let lamps = crate::hud::LAMPS.iter().chain([&crate::hud::MISSILE_LAMP]);
    for &c in SHOT_COLOURS.iter().chain(lamps) {
        set[usize::from(c)] = true;
    }
    for frame in explosion_frames {
        for &c in frame {
            set[usize::from(c)] = true;
        }
    }
    set[0] = false;
    Emissive(set)
}

/// The occluder cell holding base pixel point `p`.
fn cell_of(occ: &Occluders, p: (f32, f32)) -> (i32, i32) {
    let x = ((p.0 - occ.origin.0 as f32) / CELL as f32).floor() as i32;
    let y = ((p.1 - occ.origin.1 as f32) / CELL as f32).floor() as i32;
    (x, y)
}

/// Whether cell `c` is solid; cells outside the grid are open.
fn solid(occ: &Occluders, (x, y): (i32, i32)) -> bool {
    let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
        return false;
    };
    x < occ.cols && y < occ.rows && occ.solid.get(y * occ.cols + x) == Some(&true)
}

/// Whether any solid cell other than the light's own lies within its radius (else its
/// shadows can be skipped).
fn shadowed(occ: &Occluders, l: &Light) -> bool {
    let r = l.radius;
    let (x0, y0) = cell_of(occ, (l.pos.0 - r, l.pos.1 - r));
    let (x1, y1) = cell_of(occ, (l.pos.0 + r, l.pos.1 + r));
    let own = cell_of(occ, l.pos);
    (y0..=y1).any(|y| (x0..=x1).any(|x| (x, y) != own && solid(occ, (x, y))))
}

/// Visibility of `to` from a light at `from` (§6.2): march in steps of at most 4 px; every
/// solid cell entered multiplies by 0.25. The cells holding `to` and the light do not count.
pub fn visibility(occ: &Occluders, from: (f32, f32), to: (f32, f32)) -> f32 {
    const INV_CELL: f32 = 1.0 / CELL as f32;
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let steps = ((dx * dx + dy * dy).sqrt() / SHADOW_STEP).ceil() as u32;
    if steps < 2 {
        return 1.0;
    }
    // In cell units from the grid origin.
    let u = (from.0 - occ.origin.0 as f32) * INV_CELL;
    let v = (from.1 - occ.origin.1 as f32) * INV_CELL;
    let n = steps as f32;
    let (su, sv) = (dx * INV_CELL / n, dy * INV_CELL / n);
    let cell = |s: f32| ((u + su * s).floor() as i32, (v + sv * s).floor() as i32);
    let (own, target) = (cell(0.0), cell(n));
    let (mut last, mut vis) = (own, 1.0);
    for s in 1..steps {
        let c = cell(s as f32);
        if c != last {
            last = c;
            if c != target && c != own && solid(occ, c) {
                vis *= SHADOW_FACTOR;
                if vis <= MIN_VISIBILITY {
                    break;
                }
            }
        }
    }
    vis
}

/// Adds light `l` to half-resolution row `row` (samples at base y, x = 2i + 1).
fn add_light(row: &mut [[f32; 3]], y: f32, l: &Light, occ: Option<&Occluders>) {
    let r2 = l.radius * l.radius;
    let dy = y - l.pos.1;
    let rem = r2 - dy * dy;
    if rem <= 0.0 || row.is_empty() {
        return;
    }
    let span = rem.sqrt();
    let lo = ((l.pos.0 - span - 1.0) / 2.0).ceil().max(0.0) as usize;
    let hi = ((l.pos.0 + span - 1.0) / 2.0).floor();
    if hi < 0.0 {
        return;
    }
    let hi = (hi as usize).min(row.len() - 1);
    let k = l.color.map(|c| c * l.intensity);
    for (i, acc) in row.iter_mut().enumerate().take(hi + 1).skip(lo) {
        let x = (2 * i + 1) as f32;
        let dx = x - l.pos.0;
        let d2 = (dx * dx + dy * dy) / r2;
        if d2 >= 1.0 {
            continue;
        }
        let mut a = (1.0 - d2) * (1.0 - d2);
        if let Some(occ) = occ {
            a *= visibility(occ, l.pos, (x, y));
        }
        for c in 0..3 {
            acc[c] += k[c] * a;
        }
    }
}

/// One bilinear tap: the two source samples and the weight of the second (0..=256).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Tap {
    pub i0: u16,
    pub i1: u16,
    pub f: u16,
}

/// The tap for output pixel `x` of a grid sampled every `scale` output pixels, with sample `i`
/// centred on output coordinate `scale · (i + 0.5)`; `n` samples, edges clamped.
pub(crate) fn tap(x: usize, scale: usize, n: usize) -> Tap {
    // s = (x + 0.5) / scale − 0.5, in 1/256 units.
    let s = ((2 * x as i64 + 1 - scale as i64) * 128).div_euclid(scale as i64);
    let (i, f) = (s.div_euclid(256), s.rem_euclid(256));
    if i < 0 {
        Tap::default()
    } else if i as usize + 1 >= n {
        let last = n.saturating_sub(1) as u16;
        Tap {
            i0: last,
            i1: last,
            f: 0,
        }
    } else {
        Tap {
            i0: i as u16,
            i1: i as u16 + 1,
            f: f as u16,
        }
    }
}

/// The light map of one frame and the scratch it is built in. Reused from frame to frame
/// (no allocation in the steady state).
#[derive(Debug, Default)]
pub struct LightMap {
    /// Base frame width.
    w: usize,
    /// Half-resolution width.
    hw: usize,
    acc: Vec<[f32; 3]>,
    /// Fixed-point gains, `hw` × 80.
    gain: Vec<[u16; 3]>,
    /// Half-resolution rows whose every gain is 1.
    unlit: Vec<bool>,
    /// Per light: casts shadows that can fall in view.
    casts: Vec<bool>,
    /// Output column and row taps for (w, k).
    cols: Vec<Tap>,
    rows: Vec<Tap>,
    taps_for: (usize, usize),
}

impl LightMap {
    /// Builds the map for a frame `w` base pixels wide (§6.2).
    ///
    /// # Panics
    /// If `w` is 0 or above [`MAX_WIDTH`].
    pub fn build(&mut self, scene: &LightScene, w: usize) {
        assert!((1..=MAX_WIDTH).contains(&w), "frame width {w}");
        self.w = w;
        self.hw = w.div_ceil(2);
        let n = self.hw * HALF_ROWS;
        self.acc.resize(n, [0.0; 3]);
        let shadows = scene.mode == LightMode::Full;
        self.casts.clear();
        self.casts.extend(
            scene
                .lights
                .iter()
                .map(|l| shadows && l.shadows && shadowed(&scene.occluders, l)),
        );
        self.gain.resize(n, [0; 3]);
        self.unlit.resize(HALF_ROWS, true);
        let (occ, casts, hw) = (&scene.occluders, &self.casts, self.hw);
        let amb = ambient(scene.mode);
        self.acc
            .par_chunks_mut(hw)
            .zip(self.gain.par_chunks_mut(hw))
            .zip(self.unlit.par_iter_mut())
            .enumerate()
            .for_each(|(j, ((acc, gain), unlit))| {
                acc.fill([0.0; 3]);
                let y = (2 * j + 1) as f32;
                for (l, &cast) in scene.lights.iter().zip(casts) {
                    if l.radius > 0.0 && l.intensity > 0.0 {
                        add_light(acc, y, l, cast.then_some(occ));
                    }
                }
                for (g, a) in gain.iter_mut().zip(acc.iter()) {
                    *g = std::array::from_fn(|c| {
                        let v = (amb[c] + a[c]).min(MAX_GAIN);
                        (v * GAIN_ONE as f32 + 0.5) as u16
                    });
                }
                *unlit = gain
                    .iter()
                    .all(|g| g.iter().all(|&c| u32::from(c) == GAIN_ONE));
            });
    }

    /// Base frame width of the last build.
    pub fn width(&self) -> usize {
        self.w
    }

    /// The gain at half-resolution sample (i, j), centred on base pixel point
    /// (2i + 1, 2j + 1).
    pub fn gain(&self, i: usize, j: usize) -> [f32; 3] {
        let g = self.gain[j * self.hw + i];
        g.map(|c| f32::from(c) / GAIN_ONE as f32)
    }

    fn ensure_taps(&mut self, k: usize) {
        if self.taps_for == (self.w, k) {
            return;
        }
        self.taps_for = (self.w, k);
        self.cols.clear();
        self.cols
            .extend((0..self.w * k).map(|x| tap(x, 2 * k, self.hw)));
        self.rows.clear();
        self.rows
            .extend((0..LIT_ROWS * k).map(|y| tap(y, 2 * k, HALF_ROWS)));
    }

    /// Modulates the playfield rows of the upscaled frame in place (§6.2): `rgba` is
    /// (w·k) × (200·k) RGBA8, `indexed` the composed w × 200 frame it was upscaled from (for
    /// the emissive colours, which use max(L, 1)). Rows from 160·k on and alpha are untouched.
    ///
    /// # Panics
    /// If `w` differs from the last build or the buffers do not match it.
    pub fn apply(
        &mut self,
        rgba: &mut [u8],
        w: usize,
        k: usize,
        indexed: &[u8],
        emissive: &Emissive,
    ) {
        assert_eq!(w, self.w, "the map was built for another width");
        assert!(k >= 1, "scale");
        let ow = w * k;
        assert_eq!(rgba.len(), ow * FRAME_ROWS * k * 4, "RGBA size");
        assert_eq!(indexed.len(), w * FRAME_ROWS, "indexed size");
        self.ensure_taps(k);
        let t = srgb();
        let (hw, gain, unlit) = (self.hw, &self.gain, &self.unlit);
        let (cols, rows) = (&self.cols, &self.rows);
        let lit = &mut rgba[..LIT_ROWS * k * ow * 4];
        lit.par_chunks_exact_mut(ow * 4)
            .enumerate()
            .for_each(|(y, row)| {
                let rt = rows[y];
                let (j0, j1) = (usize::from(rt.i0), usize::from(rt.i1));
                if unlit[j0] && unlit[j1] {
                    return;
                }
                let (g0, g1) = (&gain[j0 * hw..][..hw], &gain[j1 * hw..][..hw]);
                let (a, b) = (256 - u32::from(rt.f), u32::from(rt.f));
                // The gain between half-resolution rows j0 and j1 at column i.
                let column = |i: u16| -> [u32; 3] {
                    let (p, q) = (g0[usize::from(i)], g1[usize::from(i)]);
                    std::array::from_fn(|c| (u32::from(p[c]) * a + u32::from(q[c]) * b) >> 8)
                };
                let src = &indexed[(y / k) * w..][..w];
                let mut at = None;
                let (mut v0, mut v1) = ([0; 3], [0; 3]);
                let blocks = row.chunks_exact_mut(4 * k).zip(cols.chunks_exact(k));
                for ((block, cts), &c) in blocks.zip(src) {
                    let glow = emissive.0[usize::from(c)];
                    for (px, ct) in block.chunks_exact_mut(4).zip(cts) {
                        if at != Some((ct.i0, ct.i1)) {
                            at = Some((ct.i0, ct.i1));
                            (v0, v1) = (column(ct.i0), column(ct.i1));
                        }
                        let (a, b) = (256 - u32::from(ct.f), u32::from(ct.f));
                        for c in 0..3 {
                            let mut g = (v0[c] * a + v1[c] * b) >> 8;
                            if glow {
                                g = g.max(GAIN_ONE);
                            }
                            let l = u32::from(t.lin[usize::from(px[c])]);
                            px[c] = t.enc[((l * g) >> GAIN_BITS).min(LIN_MAX) as usize];
                        }
                    }
                }
            });
    }
}

/// The lighting and bloom stage of the pipeline (§3 steps 3 and 5): the light map, then bloom,
/// applied in place to the upscaled image of a flight frame. Holds the scratch buffers of both,
/// so the steady state does not allocate.
#[derive(Debug, Default)]
pub struct LightStage {
    map: LightMap,
    bloom: Bloom,
}

impl PostStage for LightStage {
    fn apply(&mut self, out: &mut [u8], k: usize, frame: &FrameInput<'_>, cfg: &Config) {
        let Some(flight) = frame.flight else {
            return;
        };
        let fb = frame.indexed;
        let w = fb.width();
        // Flight frames are composed at the output width; anything else is not lit.
        if w != frame.width {
            return;
        }
        if let (Some(_), Some(scene)) = (cfg.lighting, flight.scene) {
            self.map.build(scene, w);
            self.map.apply(out, w, k, &fb.pixels, flight.emissive);
        }
        if cfg.bloom {
            self.bloom.apply(out, w, k, &fb.pixels, flight.emissive);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const W: usize = 320;

    fn scene(mode: LightMode, lights: Vec<Light>) -> LightScene {
        LightScene {
            mode,
            lights,
            occluders: Occluders::default(),
        }
    }

    fn light(pos: (f32, f32), radius: f32, shadows: bool) -> Light {
        Light {
            pos,
            color: [1.0, 0.55, 0.15],
            radius,
            intensity: 1.0,
            shadows,
        }
    }

    /// A deterministic test image: a w × 200 indexed frame and its RGBA expansion through a
    /// palette ramp.
    fn image(w: usize) -> (Vec<u8>, Vec<u8>, [[u8; 3]; 256]) {
        let mut pal = [[0u8; 3]; 256];
        for (i, p) in pal.iter_mut().enumerate() {
            *p = [i as u8, (i * 7 % 256) as u8, (255 - i) as u8];
        }
        let indexed: Vec<u8> = (0..w * FRAME_ROWS)
            .map(|i| ((i % w) * 3 + (i / w) * 5) as u8)
            .collect();
        let rgba = expand(&indexed, &pal, 1, w);
        (indexed, rgba, pal)
    }

    /// Nearest-neighbour upscale of an indexed frame to RGBA.
    fn expand(indexed: &[u8], pal: &[[u8; 3]; 256], k: usize, w: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(w * k * FRAME_ROWS * k * 4);
        for y in 0..FRAME_ROWS * k {
            for x in 0..w * k {
                let [r, g, b] = pal[usize::from(indexed[(y / k) * w + x / k])];
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
        out
    }

    #[test]
    fn srgb_round_trip_is_exact() {
        let t = srgb();
        for c in 0..=255u8 {
            assert_eq!(t.enc[usize::from(t.lin[usize::from(c)])], c);
            let back = (t.lin_f[usize::from(c)] * LIN_MAX as f32).round() as u16;
            assert_eq!(back, t.lin[usize::from(c)], "the two tables agree");
        }
        assert!(t.lin.windows(2).all(|p| p[0] < p[1]), "strictly increasing");
        assert!(t.enc.windows(2).all(|p| p[0] <= p[1]), "monotonic");
    }

    #[test]
    fn the_stage_lights_flight_frames_only() {
        let w = 428;
        let fb = crate::Framebuffer::with_width(w);
        let rgba = [200u8, 150, 100, 255].repeat(w * FRAME_ROWS);
        let night = scene(LightMode::Full, vec![]);
        let em = Emissive::default();
        let mut cfg = Config {
            lighting: Some(LightMode::Full),
            ..Config::default()
        };
        let flight = super::super::FlightInput {
            margin: 54,
            scene: Some(&night),
            emissive: &em,
        };
        let frame = |flight| FrameInput {
            indexed: &fb,
            width: w,
            flight,
        };
        let mut stage = LightStage::default();
        let mut out = rgba.clone();
        stage.apply(&mut out, 1, &frame(None), &cfg);
        assert_eq!(out, rgba, "front-end frames are not lit");
        stage.apply(&mut out, 1, &frame(Some(flight)), &cfg);
        let hud = LIT_ROWS * w * 4;
        assert!(
            out[..hud].chunks(4).all(|p| p[0] < 200 && p[3] == 255),
            "night"
        );
        assert_eq!(&out[hud..], &rgba[hud..], "HUD rows");
        // Bloom alone leaves a frame without bright or emissive pixels as it is.
        cfg.lighting = None;
        cfg.bloom = true;
        let mut out = rgba.clone();
        stage.apply(&mut out, 1, &frame(Some(flight)), &cfg);
        assert_eq!(out, rgba);
    }

    #[test]
    fn taps_cover_the_grid() {
        // k = 1 → scale 2: output pixel 0 (centre 0.5) is before sample 0's centre 1.
        assert_eq!(tap(0, 2, 160), Tap::default());
        assert_eq!(
            tap(1, 2, 160),
            Tap {
                i0: 0,
                i1: 1,
                f: 64
            }
        );
        assert_eq!(
            tap(2, 2, 160),
            Tap {
                i0: 0,
                i1: 1,
                f: 192
            }
        );
        assert_eq!(
            tap(319, 2, 160),
            Tap {
                i0: 159,
                i1: 159,
                f: 0
            }
        );
        // Scale 4 (bloom), sample centres at 2, 6, ...
        assert_eq!(tap(1, 4, 10), Tap::default());
        assert_eq!(
            tap(2, 4, 10),
            Tap {
                i0: 0,
                i1: 1,
                f: 32
            }
        );
        assert_eq!(
            tap(5, 4, 10),
            Tap {
                i0: 0,
                i1: 1,
                f: 224
            }
        );
    }

    #[test]
    fn no_lights_and_ambient_one_is_identity() {
        let (indexed, rgba, _) = image(W);
        let mut map = LightMap::default();
        map.build(&scene(LightMode::Subtle, vec![]), W);
        assert!((0..HALF_ROWS).all(|j| map.gain(7, j) == [1.0; 3]));
        let mut out = rgba.clone();
        map.apply(&mut out, W, 1, &indexed, &Emissive::default());
        assert_eq!(out, rgba);
        // The arithmetic itself is the identity too, not just the skipped rows.
        let far = light((-500.0, -500.0), 10.0, false);
        map.build(&scene(LightMode::Subtle, vec![far]), W);
        map.unlit.fill(false);
        let mut out = rgba.clone();
        map.apply(&mut out, W, 1, &indexed, &Emissive::default());
        assert_eq!(out, rgba);
    }

    #[test]
    fn single_light_is_radially_symmetric() {
        let mut map = LightMap::default();
        // On sample (50, 40)'s centre.
        let l = light((101.0, 81.0), 30.0, false);
        map.build(&scene(LightMode::Full, vec![l]), W);
        let c = map.gain(50, 40);
        assert!(c[0] > 1.0, "lit above the night ambient: {c:?}");
        for d in 1..15 {
            let ring = [
                map.gain(50 + d, 40),
                map.gain(50 - d, 40),
                map.gain(50, 40 + d),
                map.gain(50, 40 - d),
            ];
            assert!(ring.iter().all(|&g| g == ring[0]), "d = {d}: {ring:?}");
            let diag = [
                map.gain(50 + d, 40 + d),
                map.gain(50 - d, 40 - d),
                map.gain(50 + d, 40 - d),
                map.gain(50 - d, 40 + d),
            ];
            assert!(diag.iter().all(|&g| g == diag[0]), "d = {d}: {diag:?}");
            assert!(ring[0][0] <= map.gain(50 + d - 1, 40)[0], "falls off");
        }
        // Beyond the radius only the ambient is left.
        assert_eq!(map.gain(66, 40)[2], map.gain(100, 10)[2]);
        let amb = (NIGHT_AMBIENT[2] * GAIN_ONE as f32 + 0.5) as u16;
        assert_eq!(map.gain(100, 10)[2], f32::from(amb) / GAIN_ONE as f32);
    }

    #[test]
    fn falloff_follows_the_formula() {
        let mut map = LightMap::default();
        let l = Light {
            color: [0.5; 3],
            ..light((101.0, 81.0), 40.0, false)
        };
        map.build(&scene(LightMode::Subtle, vec![l]), W);
        // 10 samples = 20 px away: d = 0.5, falloff 0.5625, gain 1 + 0.5 · 0.5625.
        let want = 1.0 + 0.5 * 0.5625;
        let got = map.gain(60, 40)[0];
        assert!((got - want).abs() < 1.0 / 4000.0, "{got} vs {want}");
        // The cap.
        let hot = Light {
            intensity: 9.0,
            ..l
        };
        map.build(&scene(LightMode::Subtle, vec![hot]), W);
        assert!((map.gain(50, 40)[0] - MAX_GAIN).abs() < 1.0 / 4000.0);
    }

    fn wall() -> Occluders {
        // A 10 × 5 grid at the origin with one solid cell at (3, 1): x 96..128, y 32..64.
        let mut solid = vec![false; 50];
        solid[13] = true;
        Occluders {
            origin: (0, 0),
            cols: 10,
            rows: 5,
            solid,
        }
    }

    #[test]
    fn a_solid_cell_darkens_the_far_side() {
        let occ = wall();
        let pos = (60.0, 48.0);
        assert_eq!(visibility(&occ, pos, (90.0, 48.0)), 1.0, "near side");
        assert_eq!(
            visibility(&occ, pos, (110.0, 48.0)),
            1.0,
            "inside the cell itself"
        );
        assert_eq!(visibility(&occ, pos, (140.0, 48.0)), 0.25, "far side");
        assert_eq!(visibility(&occ, pos, (140.0, 10.0)), 1.0, "passes above");
        let mut map = LightMap::default();
        let l = light(pos, 100.0, true);
        let lit = |occluders: Occluders, shadows| {
            let mut map2 = LightMap::default();
            map2.build(
                &LightScene {
                    mode: LightMode::Full,
                    lights: vec![Light { shadows, ..l }],
                    occluders,
                },
                W,
            );
            map2.gain(70, 23)[0] // base (141, 47): beyond the wall
        };
        let open = lit(Occluders::default(), true);
        let blocked = lit(occ.clone(), true);
        let no_shadows = lit(occ.clone(), false);
        assert!(blocked < open, "{blocked} < {open}");
        assert_eq!(no_shadows, open, "a light without shadows ignores walls");
        // Same distance in an open direction (61, 129): unaffected.
        map.build(
            &LightScene {
                mode: LightMode::Full,
                lights: vec![l],
                occluders: occ,
            },
            W,
        );
        assert_eq!(map.gain(30, 64)[0], open, "the point below is open");
        // Subtle never casts shadows.
        let mut subtle = LightMap::default();
        subtle.build(
            &LightScene {
                mode: LightMode::Subtle,
                lights: vec![l],
                occluders: wall(),
            },
            W,
        );
        assert_eq!(subtle.gain(70, 23), subtle.gain(30, 64));
    }

    #[test]
    fn emissive_pixels_keep_their_colour() {
        let (indexed, rgba, _) = image(W);
        let emissive = flight_emissive([[0x10u8, 0x20, 0].as_slice()]);
        assert!(emissive.0[0x3D] && emissive.0[0x26] && emissive.0[0x31]);
        assert!(emissive.0[0x10] && emissive.0[0x20] && !emissive.0[0]);
        let mut map = LightMap::default();
        map.build(&scene(LightMode::Full, vec![]), W);
        let mut out = rgba.clone();
        map.apply(&mut out, W, 1, &indexed, &emissive);
        let mut darker = 0;
        for (i, &c) in indexed.iter().enumerate().take(LIT_ROWS * W) {
            let (o, r) = (&out[i * 4..i * 4 + 4], &rgba[i * 4..i * 4 + 4]);
            if emissive.0[usize::from(c)] {
                assert_eq!(o, r, "emissive pixel {i}");
            } else if o != r {
                darker += 1;
                assert!(o.iter().zip(r).all(|(o, r)| o <= r));
            }
        }
        assert!(darker > LIT_ROWS * W / 2, "the night darkens the rest");
        assert_eq!(&out[LIT_ROWS * W * 4..], &rgba[LIT_ROWS * W * 4..], "HUD");
        // Under a light an emissive pixel is never darker.
        map.build(
            &scene(LightMode::Full, vec![light((100.0, 80.0), 60.0, false)]),
            W,
        );
        let mut out = rgba.clone();
        map.apply(&mut out, W, 1, &indexed, &emissive);
        for (i, &c) in indexed.iter().enumerate().take(LIT_ROWS * W) {
            if emissive.0[usize::from(c)] {
                assert!((0..3).all(|ch| out[i * 4 + ch] >= rgba[i * 4 + ch]));
            }
        }
    }

    fn digest(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// A synthetic flight-like scene: two walls, an explosion, shots and a flame.
    pub(crate) fn synthetic(w: usize, mode: LightMode) -> LightScene {
        let m = (w as f32 - 320.0) / 2.0;
        let mut solid = vec![false; 13 * 7];
        for (x, y) in [(2, 1), (3, 1), (7, 3), (8, 3), (8, 4), (5, 0)] {
            solid[y * 13 + x] = true;
        }
        let o = |x: f32, y: f32| (x + m, y);
        LightScene {
            mode,
            lights: vec![
                Light {
                    pos: o(120.0, 70.0),
                    color: [1.0, 0.55, 0.15],
                    radius: 56.0,
                    intensity: 1.4,
                    shadows: true,
                },
                Light {
                    pos: o(200.0, 120.0),
                    color: [0.2, 0.9, 0.3],
                    radius: 14.0,
                    intensity: 0.9,
                    shadows: false,
                },
                Light {
                    pos: o(160.0, 116.0),
                    color: [0.6, 0.8, 1.0],
                    radius: 32.0,
                    intensity: 1.3,
                    shadows: true,
                },
                Light {
                    pos: o(260.0, 40.0),
                    color: [1.0, 0.9, 0.6],
                    radius: 24.0,
                    intensity: 1.2,
                    shadows: false,
                },
            ],
            occluders: Occluders {
                origin: (-24 + m as i32, -10),
                cols: 13,
                rows: 7,
                solid,
            },
        }
    }

    #[test]
    fn golden_synthetic_scene() {
        let (indexed, _, pal) = image(W);
        let emissive = flight_emissive([[0x40u8, 0x41].as_slice()]);
        let mut map = LightMap::default();
        let mut got = vec![];
        for (mode, k) in [
            (LightMode::Full, 1),
            (LightMode::Subtle, 2),
            (LightMode::Full, 3),
        ] {
            map.build(&synthetic(W, mode), W);
            let mut out = expand(&indexed, &pal, k, W);
            map.apply(&mut out, W, k, &indexed, &emissive);
            got.push(digest(&out));
        }
        assert_eq!(
            got,
            ["daa1eae47927180f", "cf3c8e279fb5ce6c", "775f2be2836050df"]
        );
    }
}
