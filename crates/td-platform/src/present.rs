//! Presenting the indexed 320x200 framebuffer in a window of any size.
//!
//! Spec: MDG §9.3.8. Aspect-correct mode shows the image at 4:3 (each source pixel 1.2x taller
//! than wide) using sharp-bilinear scaling: nearest-neighbour up to the largest integer factor,
//! then linear filtering for the remaining fraction.
//!
//! Enhanced frames (`docs/spec/enhanced/presentation.md` rev 1 §3) take a separate path,
//! [`Presenter::present_rgba`]: an RGBA image of (W·k) × (200·k) pixels, a base frame W × 200
//! upscaled k times. The same scale modes apply, with the original 5:6 pixel aspect of the base
//! frame (W : 240 in the aspect-correct mode) and integer factors of the base frame in the
//! integer mode. The Classic path above is unchanged.

use sdl3::pixels::{Color, PixelFormat};

use sdl3::render::{FRect, ScaleMode as SdlScale, Texture, TextureCreator, WindowCanvas};
use sdl3::video::WindowContext;
use td_render::{Framebuffer, HEIGHT, WIDTH};

use crate::{PlatformError, sdl_err};

/// How the framebuffer is scaled to the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleMode {
    /// 4:3 display aspect, sharp-bilinear (default; matches a VGA monitor).
    #[default]
    Aspect4x3,
    /// Square pixels at the largest integer factor that fits (pixel-perfect).
    IntegerSquare,
    /// Fill the whole window.
    Stretch,
}

pub(crate) struct Presenter {
    creator: &'static TextureCreator<WindowContext>,
    screen: Texture<'static>,
    /// Intermediate integer-scaled texture and its factor.
    upscaled: Option<(Texture<'static>, u32)>,
    rgba: Vec<u8>,
    pub(crate) mode: ScaleMode,
    /// Enhanced frames: the streaming texture and its size.
    frame: Option<(Texture<'static>, (u32, u32))>,
    /// Enhanced frames: the sharp-bilinear intermediate and its size.
    frame_up: Option<(Texture<'static>, (u32, u32))>,
}

impl Presenter {
    pub(crate) fn new(canvas: &WindowCanvas, mode: ScaleMode) -> Result<Self, PlatformError> {
        // Textures borrow their creator; the program has exactly one window, so the creator is
        // leaked to give textures a 'static lifetime.
        let creator: &'static TextureCreator<WindowContext> =
            Box::leak(Box::new(canvas.texture_creator()));
        let mut screen = creator
            .create_texture_streaming(PixelFormat::RGBA32, WIDTH as u32, HEIGHT as u32)
            .map_err(sdl_err)?;
        screen.set_scale_mode(SdlScale::Nearest);
        Ok(Self {
            creator,
            screen,
            upscaled: None,
            rgba: vec![0; WIDTH * HEIGHT * 4],
            mode,
            frame: None,
            frame_up: None,
        })
    }

    /// Shows an Enhanced frame: `rgba` holds `size` = (W·k, 200·k) pixels, `base` = (W, 200).
    pub(crate) fn present_rgba(
        &mut self,
        canvas: &mut WindowCanvas,
        rgba: &[u8],
        size: (u32, u32),
        base: (u32, u32),
        capture: bool,
    ) -> Result<Option<WindowCapture>, PlatformError> {
        let (w, h) = size;
        assert_eq!(rgba.len(), (w * h * 4) as usize, "RGBA frame size");
        if self.frame.as_ref().map(|(_, s)| *s) != Some(size) {
            let t = self
                .creator
                .create_texture_streaming(PixelFormat::RGBA32, w, h)
                .map_err(sdl_err)?;
            self.frame = Some((t, size));
        }
        let (tex, _) = self.frame.as_mut().expect("just created");
        tex.update(None, rgba, w as usize * 4).map_err(sdl_err)?;

        let (out_w, out_h) = canvas.output_size().map_err(sdl_err)?;
        let dest = dest_rect_any(self.mode, out_w, out_h, base);
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        // Largest integer factor of the image that fits the destination.
        let j = ((dest.w as u32) / w).min((dest.h as u32) / h);
        let exact = (dest.w as u32).is_multiple_of(w) && (dest.h as u32).is_multiple_of(h);
        if self.mode == ScaleMode::IntegerSquare || j < 2 || exact {
            // One step: nearest when the factor is whole, linear otherwise (also downscaling).
            tex.set_scale_mode(if exact {
                SdlScale::Nearest
            } else {
                SdlScale::Linear
            });
            canvas.copy(tex, None, dest).map_err(sdl_err)?;
        } else {
            // Sharp bilinear: nearest to j times, then linear for the remaining fraction.
            tex.set_scale_mode(SdlScale::Nearest);
            let up_size = (w * j, h * j);
            if self.frame_up.as_ref().map(|(_, s)| *s) != Some(up_size) {
                let mut t = self
                    .creator
                    .create_texture_target(PixelFormat::RGBA32, up_size.0, up_size.1)
                    .map_err(sdl_err)?;
                t.set_scale_mode(SdlScale::Linear);
                self.frame_up = Some((t, up_size));
            }
            let (up, _) = self.frame_up.as_mut().expect("just created");
            let src = &*tex;
            let mut copy_result = Ok(());
            canvas
                .with_texture_canvas(up, |c| copy_result = c.copy(src, None, None))
                .map_err(sdl_err)?;
            copy_result.map_err(sdl_err)?;
            canvas.copy(up, None, dest).map_err(sdl_err)?;
        }
        let captured = if capture {
            Some(read_window(canvas)?)
        } else {
            None
        };
        canvas.present();
        Ok(captured)
    }

    pub(crate) fn present(
        &mut self,
        canvas: &mut WindowCanvas,
        fb: &Framebuffer,
        capture: bool,
    ) -> Result<Option<WindowCapture>, PlatformError> {
        let lut = fb.palette.to_rgb8();
        for (dst, &p) in self.rgba.chunks_exact_mut(4).zip(fb.pixels.iter()) {
            let [r, g, b] = lut[p as usize];
            dst.copy_from_slice(&[r, g, b, 255]);
        }
        self.screen
            .update(None, &self.rgba, WIDTH * 4)
            .map_err(sdl_err)?;

        let (out_w, out_h) = canvas.output_size().map_err(sdl_err)?;
        let dest = dest_rect(self.mode, out_w, out_h);
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        match self.mode {
            ScaleMode::IntegerSquare => {
                canvas.copy(&self.screen, None, dest).map_err(sdl_err)?;
            }
            ScaleMode::Aspect4x3 | ScaleMode::Stretch => {
                let k = integer_factor(dest.w as u32, dest.h as u32);
                if self.upscaled.as_ref().map(|(_, f)| *f) != Some(k) {
                    let mut t = self
                        .creator
                        .create_texture_target(
                            PixelFormat::RGBA32,
                            WIDTH as u32 * k,
                            HEIGHT as u32 * k,
                        )
                        .map_err(sdl_err)?;
                    t.set_scale_mode(SdlScale::Linear);
                    self.upscaled = Some((t, k));
                }
                let (up, _) = self.upscaled.as_mut().expect("just created");
                let screen = &self.screen;
                let mut copy_result = Ok(());
                canvas
                    .with_texture_canvas(up, |c| copy_result = c.copy(screen, None, None))
                    .map_err(sdl_err)?;
                copy_result.map_err(sdl_err)?;
                canvas.copy(up, None, dest).map_err(sdl_err)?;
            }
        }
        let captured = if capture {
            Some(read_window(canvas)?)
        } else {
            None
        };
        canvas.present();
        Ok(captured)
    }
}

/// What the window shows now, as RGBA8 rows.
fn read_window(canvas: &WindowCanvas) -> Result<WindowCapture, PlatformError> {
    let surface = canvas
        .read_pixels(None)
        .map_err(sdl_err)?
        .convert_format(PixelFormat::RGBA32)
        .map_err(sdl_err)?;
    let (w, h, pitch) = (surface.width(), surface.height(), surface.pitch() as usize);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    surface.with_lock(|px| {
        for row in px.chunks(pitch).take(h as usize) {
            rgba.extend_from_slice(&row[..w as usize * 4]);
        }
    });
    Ok(WindowCapture {
        width: w,
        height: h,
        rgba,
    })
}

/// Destination rectangle for an Enhanced frame whose base frame is `base` = (W, 200): the
/// aspect mode keeps the 5:6 pixel aspect (W : 240), the integer mode uses whole multiples of
/// the base frame with square pixels. For (320, 200) this is [`dest_rect`].
fn dest_rect_any(mode: ScaleMode, out_w: u32, out_h: u32, (bw, bh): (u32, u32)) -> FRect {
    let (w, h) = (out_w as f32, out_h as f32);
    // Display aspect num / den = W / (1.2 · 200).
    let (num, den) = ((bw * 5) as f32, (bh * 6) as f32);
    let (dw, dh) = match mode {
        ScaleMode::Stretch => (w, h),
        ScaleMode::Aspect4x3 => {
            if w * den > h * num {
                (h * num / den, h)
            } else {
                (w, w * den / num)
            }
        }
        ScaleMode::IntegerSquare => {
            let k = (out_w / bw).min(out_h / bh).max(1) as f32;
            (bw as f32 * k, bh as f32 * k)
        }
    };
    FRect::new(((w - dw) / 2.0).floor(), ((h - dh) / 2.0).floor(), dw, dh)
}

/// The presented window contents, as RGBA8 rows (for automated checks of scaling).
#[derive(Debug, Clone)]
pub struct WindowCapture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Largest integer factor not exceeding the destination size (at least 1).
fn integer_factor(w: u32, h: u32) -> u32 {
    (w / WIDTH as u32).min(h / HEIGHT as u32).max(1)
}

/// Destination rectangle for a mode in an `out_w` x `out_h` window.
fn dest_rect(mode: ScaleMode, out_w: u32, out_h: u32) -> FRect {
    let (w, h) = (out_w as f32, out_h as f32);
    let (dw, dh) = match mode {
        ScaleMode::Stretch => (w, h),
        ScaleMode::Aspect4x3 => {
            if w * 3.0 > h * 4.0 {
                (h * 4.0 / 3.0, h)
            } else {
                (w, w * 3.0 / 4.0)
            }
        }
        ScaleMode::IntegerSquare => {
            let k = integer_factor(out_w, out_h) as f32;
            (WIDTH as f32 * k, HEIGHT as f32 * k)
        }
    };
    FRect::new(((w - dw) / 2.0).floor(), ((h - dh) / 2.0).floor(), dw, dh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_rect_is_four_by_three_and_centred() {
        let r = dest_rect(ScaleMode::Aspect4x3, 1920, 1080);
        assert_eq!((r.w, r.h), (1440.0, 1080.0));
        assert_eq!((r.x, r.y), (240.0, 0.0));
        let r = dest_rect(ScaleMode::Aspect4x3, 800, 1000);
        assert_eq!((r.w, r.h), (800.0, 600.0));
    }

    #[test]
    fn enhanced_frames_keep_the_pixel_aspect_of_their_base() {
        for (mode, out) in [
            (ScaleMode::Aspect4x3, (1920, 1080)),
            (ScaleMode::Aspect4x3, (800, 1000)),
            (ScaleMode::IntegerSquare, (1920, 1080)),
            (ScaleMode::Stretch, (1000, 700)),
        ] {
            let classic = dest_rect(mode, out.0, out.1);
            let any = dest_rect_any(mode, out.0, out.1, (320, 200));
            assert_eq!(
                (any.x, any.y, any.w, any.h),
                (classic.x, classic.y, classic.w, classic.h)
            );
        }
        // 16:9 base (428 x 200) displays at 428 : 240.
        let r = dest_rect_any(ScaleMode::Aspect4x3, 1920, 1080, (428, 200));
        assert_eq!((r.w, r.h), (1920.0, 1920.0 * 240.0 / 428.0));
        let r = dest_rect_any(ScaleMode::Aspect4x3, 2560, 1080, (560, 200));
        assert_eq!((r.w, r.h), (1080.0 * 560.0 / 240.0, 1080.0));
        let r = dest_rect_any(ScaleMode::IntegerSquare, 1920, 1080, (560, 200));
        assert_eq!((r.w, r.h), (1680.0, 600.0), "factor 3 of the base");
    }

    #[test]
    fn integer_mode_uses_whole_factors() {
        let r = dest_rect(ScaleMode::IntegerSquare, 1920, 1080);
        assert_eq!((r.w, r.h), (1600.0, 1000.0), "factor 5");
        assert_eq!(integer_factor(100, 100), 1);
    }
}
