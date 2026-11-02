//! The indexed 320x200 framebuffer and basic, clipped drawing primitives.
//!
//! Fidelity: T0. Clipping rules for game sprites are still to be specified
//! (`docs/spec/render/sprites.md`); these primitives clip per pixel to a rectangle.

use td_formats::palette::Palette;

/// Framebuffer width in pixels.
pub const WIDTH: usize = 320;
/// Framebuffer height in pixels.
pub const HEIGHT: usize = 200;

/// An axis-aligned rectangle in framebuffer coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    /// The whole screen.
    pub const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        w: WIDTH as i32,
        h: HEIGHT as i32,
    };

    /// Intersection with another rectangle (possibly empty).
    pub fn intersect(self, o: Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        Rect {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0),
            h: (y1 - y0).max(0),
        }
    }

    /// True if the rectangle has no area.
    pub fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

/// A borrowed row-major image of palette indices.
#[derive(Debug, Clone, Copy)]
pub struct Image<'a> {
    pub pixels: &'a [u8],
    pub w: usize,
    pub h: usize,
}

impl<'a> Image<'a> {
    /// Wraps `pixels` as a `w` x `h` image.
    ///
    /// # Panics
    /// If `pixels.len() != w * h`.
    pub fn new(pixels: &'a [u8], w: usize, h: usize) -> Self {
        assert_eq!(pixels.len(), w * h, "image size mismatch");
        Self { pixels, w, h }
    }
}

/// 320x200 palette indices plus the current palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    pub pixels: Box<[u8]>,
    pub palette: Palette,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framebuffer {
    /// A black framebuffer with an all-black palette.
    pub fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT].into_boxed_slice(),
            palette: Palette::default(),
        }
    }

    /// Fills every pixel with `color`.
    pub fn clear(&mut self, color: u8) {
        self.pixels.fill(color);
    }

    /// Palette index at (x, y), or `None` outside the screen.
    pub fn get(&self, x: i32, y: i32) -> Option<u8> {
        Self::index(x, y).map(|i| self.pixels[i])
    }

    /// Sets one pixel; ignored outside the screen.
    pub fn put(&mut self, x: i32, y: i32, color: u8) {
        if let Some(i) = Self::index(x, y) {
            self.pixels[i] = color;
        }
    }

    /// Copies a full 320x200 image.
    ///
    /// # Panics
    /// If `src` is not exactly 64,000 bytes (callers pass parsed screens).
    pub fn copy_full(&mut self, src: &[u8]) {
        self.pixels.copy_from_slice(src);
    }

    /// Draws `img` at (x, y), clipped to `clip`. When `transparent` is `Some(key)`, pixels
    /// equal to `key` are skipped.
    pub fn blit(&mut self, img: Image<'_>, x: i32, y: i32, clip: Rect, transparent: Option<u8>) {
        let (src, w, h) = (img.pixels, img.w, img.h);
        let area = Rect {
            x,
            y,
            w: w as i32,
            h: h as i32,
        }
        .intersect(clip.intersect(Rect::SCREEN));
        if area.is_empty() {
            return;
        }
        for dy in area.y..area.y + area.h {
            let sy = (dy - y) as usize;
            let row = &src[sy * w..(sy + 1) * w];
            let dst_row = dy as usize * WIDTH;
            for dx in area.x..area.x + area.w {
                let c = row[(dx - x) as usize];
                if transparent != Some(c) {
                    self.pixels[dst_row + dx as usize] = c;
                }
            }
        }
    }

    /// The framebuffer expanded to RGBA8 with the current palette (for presentation and tools).
    pub fn to_rgba(&self) -> Vec<u8> {
        let lut = self.palette.to_rgb8();
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &p in self.pixels.iter() {
            let [r, g, b] = lut[p as usize];
            out.extend_from_slice(&[r, g, b, 255]);
        }
        out
    }

    fn index(x: i32, y: i32) -> Option<usize> {
        ((0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y))
            .then(|| y as usize * WIDTH + x as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blit_clips_and_honours_transparency() {
        let mut fb = Framebuffer::new();
        fb.clear(9);
        let src = [1, 0, 2, 3];
        fb.blit(Image::new(&src, 2, 2), -1, -1, Rect::SCREEN, Some(0));
        assert_eq!(
            fb.get(0, 0),
            Some(3),
            "only the bottom-right pixel is on screen"
        );
        fb.blit(Image::new(&src, 2, 2), 10, 10, Rect::SCREEN, Some(0));
        assert_eq!(
            [
                fb.get(10, 10),
                fb.get(11, 10),
                fb.get(10, 11),
                fb.get(11, 11)
            ],
            [Some(1), Some(9), Some(2), Some(3)]
        );
        fb.blit(Image::new(&src, 2, 2), 318, 198, Rect::SCREEN, None);
        assert_eq!(fb.get(319, 199), Some(3));
        let clip = Rect {
            x: 0,
            y: 0,
            w: 50,
            h: 50,
        };
        fb.blit(Image::new(&src, 2, 2), 49, 49, clip, None);
        assert_eq!((fb.get(49, 49), fb.get(50, 50)), (Some(1), Some(9)));
    }

    #[test]
    fn rgba_uses_palette() {
        let mut fb = Framebuffer::new();
        fb.palette.colors[5] = td_formats::palette::Rgb6 { r: 63, g: 0, b: 32 };
        fb.put(0, 0, 5);
        assert_eq!(&fb.to_rgba()[..4], &[255, 0, 130, 255]);
    }
}
