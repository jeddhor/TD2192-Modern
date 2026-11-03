//! Proportional text with the small font (`td.lt2`): dialogue lines, speaker names and prompts.
//!
//! Observed in the original (oracle C-0002 dialogue frames, `docs/re/dynamic-observations.md`
//! §12; T0 for the observed lines): each glyph is trimmed to its inked columns and followed by a
//! 1-pixel gap; a space advances 4 pixels; every pixel is drawn in one solid colour. The font
//! file has letters and digits only. The punctuation shapes below are the ones seen on screen
//! (the original draws them from elsewhere; RE pending, U-037). Characters with no known glyph
//! advance like a space.

use td_formats::font::{Font, FontKind};

use crate::framebuffer::{Framebuffer, Image, Rect};

/// Glyph height in pixels.
pub const HEIGHT: i32 = 5;
/// Advance of a space in pixels.
pub const SPACE_ADVANCE: i32 = 4;
/// Blank columns after each glyph.
pub const GAP: i32 = 1;

/// Punctuation seen on screen: (character, width, 5 rows as bit strings, leftmost bit first).
/// The data's `"` is drawn as an apostrophe.
const PUNCTUATION: &[(char, usize, [&str; 5])] = &[
    ('.', 1, ["0", "0", "0", "0", "1"]),
    (',', 2, ["00", "00", "00", "01", "10"]),
    ('"', 2, ["01", "10", "00", "00", "00"]),
    ('\'', 2, ["01", "10", "00", "00", "00"]),
    ('!', 1, ["1", "1", "1", "0", "1"]),
];

/// A glyph trimmed to its inked columns (1 = ink).
fn glyph_mask(font: &Font, c: char) -> Option<(usize, Vec<u8>)> {
    if let Some(&(_, w, rows)) = PUNCTUATION.iter().find(|p| p.0 == c) {
        let mut m = Vec::with_capacity(w * 5);
        for r in rows {
            m.extend(r.bytes().map(|b| u8::from(b == b'1')));
        }
        return Some((w, m));
    }
    let g = font.glyph(c)?;
    let (w, h) = font.kind.size();
    let inked: Vec<usize> = (0..w)
        .filter(|&x| (0..h).any(|y| g[y * w + x] != 0))
        .collect();
    let (&first, &last) = (inked.first()?, inked.last()?);
    let tw = last - first + 1;
    let mut m = Vec::with_capacity(tw * h);
    for y in 0..h {
        m.extend((first..=last).map(|x| u8::from(g[y * w + x] != 0)));
    }
    Some((tw, m))
}

/// Width in pixels of `text` (including the gap after the last glyph).
pub fn width(font: &Font, text: &str) -> i32 {
    text.chars()
        .map(|c| match glyph_mask(font, c) {
            Some((w, _)) if c != ' ' => w as i32 + GAP,
            _ => SPACE_ADVANCE,
        })
        .sum()
}

/// Draws `text` at (`x`, `y`) in palette index `color`; returns the x after the text.
///
/// # Panics
/// If `font` is not the small font.
pub fn draw(
    fb: &mut Framebuffer,
    font: &Font,
    text: &str,
    (x, y): (i32, i32),
    color: u8,
    clip: Rect,
) -> i32 {
    assert_eq!(font.kind, FontKind::Small, "proportional text uses td.lt2");
    let mut cx = x;
    for c in text.chars() {
        match glyph_mask(font, c) {
            Some((w, mask)) if c != ' ' => {
                let pixels: Vec<u8> = mask
                    .iter()
                    .map(|&b| if b == 1 { color } else { 0 })
                    .collect();
                fb.blit(
                    Image::new(&pixels, w, HEIGHT as usize),
                    cx,
                    y,
                    clip,
                    Some(0),
                );
                cx += w as i32 + GAP;
            }
            _ => cx += SPACE_ADVANCE,
        }
    }
    cx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_font() -> Font {
        // Glyph 'a' = a 3-wide block in columns 1..=3; every other glyph empty.
        let mut data = vec![0u8; FontKind::Small.file_len()];
        for y in 0..5 {
            for x in 1..=3 {
                data[y * 5 + x] = 0xF7;
            }
        }
        Font::parse(&data, FontKind::Small).unwrap()
    }

    #[test]
    fn proportional_layout() {
        let f = test_font();
        assert_eq!(width(&f, "a"), 4);
        assert_eq!(width(&f, "a a"), 4 + 4 + 4);
        assert_eq!(width(&f, "a."), 4 + 2);
        let mut fb = Framebuffer::new();
        let end = draw(&mut fb, &f, "  a,", (4, 10), 9, Rect::SCREEN);
        assert_eq!(end, 4 + 8 + 4 + 3);
        assert_eq!(
            fb.get(12, 10),
            Some(9),
            "trimmed glyph starts at its first inked column"
        );
        assert_eq!(fb.get(15, 10), Some(0), "1-pixel gap");
        assert_eq!(fb.get(17, 13), Some(9), "comma pixel (column 1, row 3)");
    }
}
