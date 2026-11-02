//! Full-screen images: a 768-byte palette followed by 320x200 8bpp pixels.
//!
//! Spec: `docs/re/FORMATS.md` §2 (CONFIRMED) and §3 (help screens, LIKELY). Fidelity: T0.
//! Files: every `.scr`, `city.td`, `javelin.td`; `td.hlp` holds a palette and two pages.

use crate::palette::{PALETTE_LEN, Palette};
use crate::{FormatError, Result};

/// Screen width in pixels (VGA mode 13h).
pub const WIDTH: usize = 320;
/// Screen height in pixels.
pub const HEIGHT: usize = 200;
/// Pixels in one screen.
pub const PIXELS: usize = WIDTH * HEIGHT;
/// Size of a full-screen image file.
pub const SCREEN_FILE_LEN: usize = PALETTE_LEN + PIXELS;

/// A full-screen image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub palette: Palette,
    /// 320x200 palette indices, row-major from the top-left corner.
    pub pixels: Box<[u8]>,
}

impl Screen {
    /// Parses a 64,768-byte full-screen file.
    pub fn parse(data: &[u8]) -> Result<Self> {
        FormatError::check_size("screen", data, SCREEN_FILE_LEN)?;
        Ok(Self {
            palette: Palette::parse_prefix(data)?,
            pixels: data[PALETTE_LEN..].into(),
        })
    }

    /// Palette index at (x, y), or `None` outside the screen.
    pub fn pixel(&self, x: usize, y: usize) -> Option<u8> {
        (x < WIDTH && y < HEIGHT).then(|| self.pixels[y * WIDTH + x])
    }
}

/// A palette followed by several 320x200 pages (the help file `td.hlp`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedScreens {
    pub palette: Palette,
    pub pages: Vec<Box<[u8]>>,
}

impl PagedScreens {
    /// Parses a palette and as many whole pages as the data holds (at least one).
    pub fn parse(data: &[u8]) -> Result<Self> {
        let body = data.len().saturating_sub(PALETTE_LEN);
        if body == 0 || !body.is_multiple_of(PIXELS) {
            return Err(FormatError::Invalid {
                format: "paged screens",
                offset: 0,
                detail: format!("length {} is not 768 + n * 64000", data.len()),
            });
        }
        Ok(Self {
            palette: Palette::parse_prefix(data)?,
            pages: data[PALETTE_LEN..]
                .chunks_exact(PIXELS)
                .map(Into::into)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(pages: usize) -> Vec<u8> {
        let mut data = vec![0u8; PALETTE_LEN + pages * PIXELS];
        data[PALETTE_LEN + WIDTH + 2] = 7; // (2, 1)
        data
    }

    #[test]
    fn parses_synthetic_screen() {
        let s = Screen::parse(&synthetic(1)).unwrap();
        assert_eq!(s.pixel(2, 1), Some(7));
        assert_eq!(s.pixel(320, 0), None);
        assert!(Screen::parse(&synthetic(2)).is_err());
    }

    #[test]
    fn parses_paged_screens() {
        assert_eq!(PagedScreens::parse(&synthetic(2)).unwrap().pages.len(), 2);
        assert!(PagedScreens::parse(&[0u8; PALETTE_LEN]).is_err());
        assert!(PagedScreens::parse(&[0u8; PALETTE_LEN + 100]).is_err());
    }
}
