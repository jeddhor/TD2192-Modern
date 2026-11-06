//! The Enhanced presentation pipeline (`docs/spec/enhanced/presentation.md` rev 1, ADR-0006):
//! widescreen, pixel-art upscaling, and the hook for dynamic lighting and bloom.
//!
//! Fidelity: T3 (intentional deviation, Enhanced profile only). Nothing here runs in the Classic
//! path, and nothing here feeds back into the simulation.
//!
//! The stages, in order (§3):
//!
//! 1. **compose**: the screen draws its indexed frame, W x 200. The flight screen draws a
//!    W-wide [`Framebuffer`] through the viewport functions of [`crate::flight`] and
//!    [`widescreen`]; every other screen draws the Classic 320 x 200 frame, which stage 2
//!    centres with black side panels (§4.3).
//! 2. **palette to RGBA** (W x 200), with the HUD band's side panels darkened by half (§4.3).
//! 3. **upscale** by k = 1..4 ([`upscale`], §5), with a hard edge at row 160 in flight frames.
//! 4. **lighting and bloom** (§6, M12-11/M12-12): a [`PostStage`] applied in place to the
//!    upscaled image of flight frames.
//! 5. platform scaling of the (W·k) x (200·k) image, in `td-platform`.
//!
//! With every Enhanced key off, the output is the Classic RGBA ([`Framebuffer::to_rgba`]), which
//! the tests check. [`Pipeline`] keeps its buffers between frames, so the steady state does not
//! allocate.

pub mod scene;
pub mod upscale;
pub mod widescreen;

use crate::flight::HUD_Y;
use crate::framebuffer::{Framebuffer, HEIGHT, WIDTH};
use scene::{Emissive, LightMode, LightScene};

pub use upscale::Upscaler;

/// Base widths of the widescreen presets (§4.1): off, 16:10, 16:9, 21:9.
pub const PRESET_WIDTHS: [usize; 4] = [320, 384, 428, 560];

/// Height of the HUD bar (rows 160..192).
const HUD_ROWS: std::ops::Range<usize> = HUD_Y as usize..HUD_Y as usize + 32;

/// The margin m = (W − 320) / 2 on each side of the Classic picture (§4.1).
pub fn margin(width: usize) -> usize {
    width.saturating_sub(WIDTH) / 2
}

/// The widest preset whose display aspect does not exceed the window's (`widescreen = "auto"`,
/// §2). A W x 200 frame displays at W : 240 with the original 5:6 pixel aspect, or W : 200 with
/// square pixels. Aspects within 1% count as equal: the 16:9 preset, 428 : 240 = 1.783, is
/// slightly wider than 16:9 because W keeps m = (W − 320) / 2 whole and even.
pub fn auto_width(window_w: u32, window_h: u32, square_pixels: bool) -> usize {
    let rows: u64 = if square_pixels { 200 } else { 240 };
    PRESET_WIDTHS
        .into_iter()
        .filter(|&w| 100 * w as u64 * u64::from(window_h) <= 101 * rows * u64::from(window_w))
        .max()
        .unwrap_or(WIDTH)
}

/// The stage settings of one frame (the effective `[enhanced]` section, §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Config {
    pub upscaler: Upscaler,
    /// `None` when lighting is off.
    pub lighting: Option<LightMode>,
    pub bloom: bool,
}

/// What the flight screen supplies with its frame.
#[derive(Debug, Clone, Copy)]
pub struct FlightInput<'a> {
    /// The widescreen margin m: the Classic picture's column 0 is frame column m.
    pub margin: usize,
    /// Light sources and occluders (§6.1), when lighting is on.
    pub scene: Option<&'a LightScene>,
    pub emissive: &'a Emissive,
}

/// One composed frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameInput<'a> {
    /// The indexed frame: `width` wide, or 320 wide for screens that only have the Classic
    /// picture (centred in `width` with black side panels).
    pub indexed: &'a Framebuffer,
    /// The base width W of the output.
    pub width: usize,
    /// Set for in-flight frames (not the flight's modal screens).
    pub flight: Option<FlightInput<'a>>,
}

/// The lighting and bloom stages (M12-11, M12-12): run after the upscale on flight frames, in
/// place on `out`, the (W·k) x (200·k) RGBA8 image (4 bytes per pixel, row-major).
pub trait PostStage {
    fn apply(&mut self, out: &mut [u8], k: usize, frame: &FrameInput<'_>, cfg: &Config);
}

/// No lighting and no bloom.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPost;

impl PostStage for NoPost {
    fn apply(&mut self, _: &mut [u8], _: usize, _: &FrameInput<'_>, _: &Config) {}
}

/// A finished frame: RGBA8 rows of `width` pixels.
#[derive(Debug, Clone, Copy)]
pub struct Rendered<'a> {
    pub rgba: &'a [u8],
    pub width: usize,
    pub height: usize,
}

/// The pipeline and its reusable buffers.
#[derive(Debug, Default)]
pub struct Pipeline {
    base: Vec<[u8; 4]>,
    out: Vec<[u8; 4]>,
    scratch: upscale::Scratch,
}

impl Pipeline {
    /// Runs stages 2..4 on `frame`.
    ///
    /// # Panics
    /// If the indexed frame is wider than `frame.width`.
    pub fn render(
        &mut self,
        frame: &FrameInput<'_>,
        cfg: &Config,
        post: &mut dyn PostStage,
    ) -> Rendered<'_> {
        let w = frame.width;
        palette_to_rgba(frame, &mut self.base);
        let k = cfg.upscaler.factor();
        // HUD and playfield are filtered as separate images in flight frames (§5).
        let split = frame.flight.map(|_| HUD_Y as usize);
        let out = if k == 1 {
            &mut self.base
        } else {
            upscale::upscale(
                &self.base,
                (w, HEIGHT),
                split,
                cfg.upscaler,
                &mut self.out,
                &mut self.scratch,
            );
            &mut self.out
        };
        if frame.flight.is_some() && (cfg.lighting.is_some() || cfg.bloom) {
            post.apply(out.as_flattened_mut(), k, frame, cfg);
        }
        Rendered {
            rgba: out.as_flattened(),
            width: w * k,
            height: HEIGHT * k,
        }
    }
}

/// Stage 2: the indexed frame through its palette into `out` (W x 200), centred with black
/// side panels when it is narrower, and the HUD band's side panels at half brightness (§4.3).
fn palette_to_rgba(frame: &FrameInput<'_>, out: &mut Vec<[u8; 4]>) {
    let (w, fb) = (frame.width, frame.indexed);
    let iw = fb.width();
    assert!(iw <= w, "indexed frame wider than the output");
    let lut = fb.palette.to_rgb8().map(|[r, g, b]| [r, g, b, 255]);
    out.clear();
    out.resize(w * HEIGHT, [0, 0, 0, 255]);
    let left = (w - iw) / 2;
    for (dst, src) in out.chunks_exact_mut(w).zip(fb.pixels.chunks_exact(iw)) {
        for (d, &p) in dst[left..left + iw].iter_mut().zip(src) {
            *d = lut[usize::from(p)];
        }
    }
    let m = frame.flight.map_or(0, |f| f.margin);
    if m > 0 && iw == w {
        for row in out[HUD_ROWS.start * w..HUD_ROWS.end * w].chunks_exact_mut(w) {
            let (l, rest) = row.split_at_mut(m);
            let centre = WIDTH.min(rest.len());
            let r = &mut rest[centre..];
            for p in l.iter_mut().chain(r) {
                *p = [p[0] >> 1, p[1] >> 1, p[2] >> 1, 255];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_formats::palette::Rgb6;

    fn test_frame(width: usize) -> Framebuffer {
        let mut fb = Framebuffer::with_width(width);
        for (i, c) in fb.palette.colors.iter_mut().enumerate() {
            let v = i as u8;
            *c = Rgb6 {
                r: v & 63,
                g: (v >> 2) & 63,
                b: (v.wrapping_mul(7)) & 63,
            };
        }
        for (i, p) in fb.pixels.iter_mut().enumerate() {
            *p = (i * 31 % 251) as u8;
        }
        fb
    }

    #[test]
    fn everything_off_is_the_classic_rgba() {
        let fb = test_frame(WIDTH);
        let mut pipe = Pipeline::default();
        let frame = FrameInput {
            indexed: &fb,
            width: WIDTH,
            flight: None,
        };
        let out = pipe.render(&frame, &Config::default(), &mut NoPost);
        assert_eq!((out.width, out.height), (320, 200));
        assert_eq!(out.rgba, fb.to_rgba().as_slice());
        // A flight frame without a margin is the Classic RGBA too.
        let emissive = Emissive::default();
        let frame = FrameInput {
            flight: Some(FlightInput {
                margin: 0,
                scene: None,
                emissive: &emissive,
            }),
            ..frame
        };
        let out = pipe.render(&frame, &Config::default(), &mut NoPost);
        assert_eq!(out.rgba, fb.to_rgba().as_slice());
    }

    #[test]
    fn classic_screens_are_centred_with_black_panels() {
        let fb = test_frame(WIDTH);
        let classic = fb.to_rgba();
        let mut pipe = Pipeline::default();
        let frame = FrameInput {
            indexed: &fb,
            width: 428,
            flight: None,
        };
        let out = pipe.render(&frame, &Config::default(), &mut NoPost);
        assert_eq!((out.width, out.height), (428, 200));
        for y in 0..200 {
            let row = &out.rgba[y * 428 * 4..(y + 1) * 428 * 4];
            assert!(row[..54 * 4].chunks(4).all(|p| p == [0, 0, 0, 255]));
            assert!(row[374 * 4..].chunks(4).all(|p| p == [0, 0, 0, 255]));
            assert_eq!(&row[54 * 4..374 * 4], &classic[y * 1280..(y + 1) * 1280]);
        }
    }

    #[test]
    fn hud_side_panels_are_darkened() {
        let fb = test_frame(384);
        let full = fb.to_rgba();
        let emissive = Emissive::default();
        let frame = FrameInput {
            indexed: &fb,
            width: 384,
            flight: Some(FlightInput {
                margin: 32,
                scene: None,
                emissive: &emissive,
            }),
        };
        let mut pipe = Pipeline::default();
        let out = pipe.render(&frame, &Config::default(), &mut NoPost);
        let px = |buf: &[u8], x: usize, y: usize| {
            let i = (y * 384 + x) * 4;
            [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
        };
        for (x, y) in [(0, 160), (31, 191), (352, 170), (383, 191)] {
            let p = px(&full, x, y);
            assert_eq!(px(out.rgba, x, y), [p[0] >> 1, p[1] >> 1, p[2] >> 1, 255]);
        }
        for (x, y) in [(32, 160), (351, 191), (0, 159), (0, 192), (383, 199)] {
            assert_eq!(px(out.rgba, x, y), px(&full, x, y), "({x}, {y}) unchanged");
        }
    }

    #[test]
    fn upscaled_output_has_k_times_the_size_and_post_runs_on_flight_frames() {
        #[derive(Default)]
        struct Count(usize, usize);
        impl PostStage for Count {
            fn apply(&mut self, out: &mut [u8], k: usize, f: &FrameInput<'_>, _: &Config) {
                self.0 += 1;
                self.1 = out.len() / (4 * f.width * k);
            }
        }
        let fb = test_frame(WIDTH);
        let emissive = Emissive::default();
        let cfg = Config {
            upscaler: Upscaler::Scale3x,
            lighting: Some(LightMode::Full),
            bloom: false,
        };
        let mut pipe = Pipeline::default();
        let mut post = Count::default();
        let menu = FrameInput {
            indexed: &fb,
            width: WIDTH,
            flight: None,
        };
        let out = pipe.render(&menu, &cfg, &mut post);
        assert_eq!(
            (out.width, out.height, out.rgba.len()),
            (960, 600, 960 * 600 * 4)
        );
        assert_eq!(post.0, 0, "front-end frames are not lit");
        let flight = FrameInput {
            flight: Some(FlightInput {
                margin: 0,
                scene: None,
                emissive: &emissive,
            }),
            ..menu
        };
        pipe.render(&flight, &cfg, &mut post);
        assert_eq!((post.0, post.1), (1, 600));
    }

    #[test]
    fn auto_picks_the_widest_preset_that_fits() {
        assert_eq!(auto_width(1280, 960, false), 320);
        assert_eq!(auto_width(1920, 1200, false), 384);
        assert_eq!(auto_width(1920, 1080, false), 428);
        assert_eq!(
            auto_width(1920, 1100, false),
            384,
            "narrower than 16:9 by more than 1%"
        );
        assert_eq!(auto_width(3440, 1440, false), 560);
        assert_eq!(auto_width(2560, 1080, false), 560);
        assert_eq!(auto_width(1000, 1000, false), 320);
        assert_eq!(
            auto_width(1920, 1080, true),
            320,
            "square pixels: 384:200 > 16:9"
        );
        assert_eq!(margin(560), 120);
        assert_eq!(margin(320), 0);
    }
}
