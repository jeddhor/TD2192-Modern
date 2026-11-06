//! Unchained VGA memory (mode 13h with the chain-4 bit off): four 64 KB planes, a CRTC start
//! address, and the screen read out of it.
//!
//! Spec: `docs/spec/flow/intro.md` (rev 1) §2 (VGA memory, rows, start address) and §4.2 (put
//! pixel, clear page). Fidelity: T0. The intro (and Episode 1, `docs/re/asset-load-sites.md`
//! §1.1) use 88-byte rows, so a row is 352 pixels wide: pixel (X, Y) is byte
//! `(Y × 88 + X div 4) mod 65536` of plane `X mod 4`. Every offset wraps at 64 KB, which is what
//! makes the end-of-flyover sprite fragment (OB-058) appear where it does.
//!
//! The visible screen is 320 × 200 pixels starting at the start address, without pel panning:
//! screen pixel (x, y) is byte `start + y × 88 + x div 4` of plane `x mod 4`.

use crate::framebuffer::{Canvas, Framebuffer, HEIGHT, WIDTH};

/// Bytes in one plane.
pub const PLANE_BYTES: usize = 65_536;
/// Bytes per row in every plane (the CRTC offset register gives 44 words). Spec: intro.md §2.
pub const ROW_BYTES: u16 = 88;
/// Pixels per row.
pub const ROW_PIXELS: usize = ROW_BYTES as usize * 4;

/// Byte offset of row `row`, wrapped to 64 KB.
#[must_use]
pub const fn row_offset(row: u16) -> u16 {
    row.wrapping_mul(ROW_BYTES)
}

/// Four planes of video memory and the CRTC start address.
#[derive(Clone, PartialEq, Eq)]
pub struct VgaMemory {
    /// Plane-major: plane `p`'s byte `o` is `planes[p * PLANE_BYTES + o]`.
    planes: Box<[u8]>,
    start: u16,
}

impl std::fmt::Debug for VgaMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VgaMemory")
            .field("start", &format_args!("{:04X}h", self.start))
            .finish_non_exhaustive()
    }
}

impl Default for VgaMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl VgaMemory {
    /// Cleared memory (every byte 0), start address 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            planes: vec![0; 4 * PLANE_BYTES].into_boxed_slice(),
            start: 0,
        }
    }

    /// Byte `offset` of `plane` (0..=3).
    #[must_use]
    pub fn byte(&self, plane: u8, offset: u16) -> u8 {
        self.planes[usize::from(plane & 3) * PLANE_BYTES + usize::from(offset)]
    }

    /// Writes byte `offset` of `plane` (0..=3).
    pub fn set_byte(&mut self, plane: u8, offset: u16, value: u8) {
        self.planes[usize::from(plane & 3) * PLANE_BYTES + usize::from(offset)] = value;
    }

    /// The byte offset and plane of pixel (`x`, `y`): `(y × 88 + x div 4) mod 65536`, plane
    /// `x mod 4` (intro §4.2, put pixel). Negative coordinates wrap the same way.
    #[must_use]
    pub fn address(x: i32, y: i32) -> (u16, u8) {
        let offset = y.wrapping_mul(i32::from(ROW_BYTES)) + x.div_euclid(4);
        (offset as u16, x.rem_euclid(4) as u8)
    }

    /// Pixel (`x`, `y`) in 352-pixel rows.
    #[must_use]
    pub fn pixel(&self, x: i32, y: i32) -> u8 {
        let (o, p) = Self::address(x, y);
        self.byte(p, o)
    }

    /// Sets pixel (`x`, `y`) in 352-pixel rows (`0957:0375`).
    pub fn set_pixel(&mut self, x: i32, y: i32, c: u8) {
        let (o, p) = Self::address(x, y);
        self.set_byte(p, o, c);
    }

    /// Writes `pixels` as one linear run from byte `offset`: pixel k goes to byte
    /// `offset + k div 4` of plane `k mod 4` (wrapping at 64 KB). This is how a file read in
    /// 352-pixel rows lands in memory (`td.blk`, `td.dun`, intro §3.1).
    pub fn load_linear(&mut self, offset: u16, pixels: &[u8]) {
        for (k, &c) in pixels.iter().enumerate() {
            let o = offset.wrapping_add((k / 4) as u16);
            self.set_byte((k % 4) as u8, o, c);
        }
    }

    /// Writes an image `width` pixels wide, row by row, with its top-left pixel at (0, `row`).
    pub fn load_rows(&mut self, row: u16, width: usize, pixels: &[u8]) {
        if width == 0 {
            return;
        }
        for (r, line) in pixels.chunks_exact(width).enumerate() {
            let y = i32::from(row) + r as i32;
            for (x, &c) in line.iter().enumerate() {
                self.set_pixel(x as i32, y, c);
            }
        }
    }

    /// Sets `len` bytes from `offset` in every plane to `value` (the intro's clear page,
    /// `0957:0170`).
    pub fn fill(&mut self, offset: u16, len: usize, value: u8) {
        for k in 0..len {
            let o = offset.wrapping_add(k as u16);
            for p in 0..4 {
                self.set_byte(p, o, value);
            }
        }
    }

    /// A latched block copy (write mode 1): for each of `rows` rows, `width` bytes of all four
    /// planes go from `src + row × src_stride` to `dst + row × dst_stride` (offsets wrap).
    pub fn copy_latched(
        &mut self,
        src: u16,
        dst: u16,
        rows: u16,
        width: u16,
        src_stride: u16,
        dst_stride: u16,
    ) {
        for r in 0..rows {
            let s = src.wrapping_add(r.wrapping_mul(src_stride));
            let d = dst.wrapping_add(r.wrapping_mul(dst_stride));
            for k in 0..width {
                for p in 0..4 {
                    let v = self.byte(p, s.wrapping_add(k));
                    self.set_byte(p, d.wrapping_add(k), v);
                }
            }
        }
    }

    /// The CRTC start address.
    #[must_use]
    pub fn start(&self) -> u16 {
        self.start
    }

    /// Sets the CRTC start address (shown from the next frame on in the original).
    pub fn set_start(&mut self, start: u16) {
        self.start = start;
    }

    /// The visible 320 × 200 pixels into `fb.pixels` (the palette is the caller's).
    pub fn render(&self, fb: &mut Framebuffer) {
        for y in 0..HEIGHT {
            let row = self.start.wrapping_add((y as u16).wrapping_mul(ROW_BYTES));
            let line = &mut fb.pixels[y * WIDTH..(y + 1) * WIDTH];
            for (x, px) in line.iter_mut().enumerate() {
                *px = self.byte((x % 4) as u8, row.wrapping_add((x / 4) as u16));
            }
        }
    }
}

/// Text and other per-pixel drawing in 352-pixel rows; nothing is clipped, offsets wrap.
impl Canvas for VgaMemory {
    fn put(&mut self, x: i32, y: i32, color: u8) {
        self.set_pixel(x, y, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_addressing_and_wrap() {
        // intro §2: (X, Y) is byte (Y × 88 + X div 4) mod 65536 of plane X mod 4.
        assert_eq!(VgaMemory::address(5, 2), (2 * 88 + 1, 1));
        assert_eq!(VgaMemory::address(351, 0), (87, 3));
        // Row 745 starts at 65,560, which wraps to byte 24.
        assert_eq!(VgaMemory::address(0, 745), (24, 0));
        let mut v = VgaMemory::new();
        v.set_pixel(0, 745, 9);
        assert_eq!(v.pixel(96, 0), 9, "same byte as (96, 0)");
    }

    #[test]
    fn linear_loads_are_352_pixel_rows() {
        let mut v = VgaMemory::new();
        let data: Vec<u8> = (0..=255u8).cycle().take(352 * 2).collect();
        v.load_linear(row_offset(10), &data);
        assert_eq!(v.pixel(0, 10), 0);
        assert_eq!(v.pixel(351, 10), data[351]);
        assert_eq!(v.pixel(3, 11), data[355]);
        // A run that passes the end of memory continues at byte 0.
        v.load_linear(0xFFFF, &[1, 2, 3, 4, 5]);
        assert_eq!((v.byte(0, 0xFFFF), v.byte(3, 0xFFFF)), (1, 4));
        assert_eq!(v.byte(0, 0), 5);
    }

    #[test]
    fn latched_copy_moves_all_planes() {
        let mut v = VgaMemory::new();
        for p in 0..4 {
            v.set_byte(p, 100, 10 + p);
            v.set_byte(p, 108, 20 + p);
        }
        v.copy_latched(100, 1000, 2, 1, 8, ROW_BYTES);
        for p in 0..4 {
            assert_eq!(v.byte(p, 1000), 10 + p);
            assert_eq!(v.byte(p, 1000 + ROW_BYTES), 20 + p);
        }
    }

    #[test]
    fn screen_reads_from_the_start_address() {
        let mut v = VgaMemory::new();
        v.set_pixel(7, 203, 42);
        v.set_start(row_offset(200));
        let mut fb = Framebuffer::new();
        v.render(&mut fb);
        assert_eq!(fb.get(7, 3), Some(42));
        v.fill(row_offset(200), 88 * 10, 0);
        v.render(&mut fb);
        assert_eq!(fb.get(7, 3), Some(0));
    }
}
