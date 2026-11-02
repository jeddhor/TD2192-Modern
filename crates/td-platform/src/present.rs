//! Presenting the indexed 320x200 framebuffer in a window of any size.
//!
//! Spec: MDG §9.3.8. Aspect-correct mode shows the image at 4:3 (each source pixel 1.2x taller
//! than wide) using sharp-bilinear scaling: nearest-neighbour up to the largest integer factor,
//! then linear filtering for the remaining fraction.

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
        })
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
            Some(WindowCapture {
                width: w,
                height: h,
                rgba,
            })
        } else {
            None
        };
        canvas.present();
        Ok(captured)
    }
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
    fn integer_mode_uses_whole_factors() {
        let r = dest_rect(ScaleMode::IntegerSquare, 1920, 1080);
        assert_eq!((r.w, r.h), (1600.0, 1000.0), "factor 5");
        assert_eq!(integer_factor(100, 100), 1);
    }
}
