//! Drawing text with the original bitmap fonts.
//!
//! Fidelity: T0 for glyph pixels; layout is provisional. ASSUMPTION(M2-08): glyphs advance by
//! their width plus 1 pixel, a space advances by the glyph width + 1, and characters without a
//! glyph are skipped like spaces. The text routines in the executables decide the real
//! spacing, colouring and wrapping (`docs/spec/render/text.md`, to be written).

use td_formats::font::Font;

use crate::framebuffer::{Framebuffer, Image, Rect};

/// Gap between glyphs in pixels (provisional, see module docs).
pub const GLYPH_GAP: i32 = 1;

/// How glyph pixels are coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextColor {
    /// Use the colours baked into the glyphs.
    #[default]
    Baked,
    /// Replace every non-zero glyph pixel with this palette index.
    Solid(u8),
}

/// Width in pixels that `text` occupies.
pub fn text_width(font: &Font, text: &str) -> i32 {
    let (w, _) = font.kind.size();
    let n = text.chars().count() as i32;
    if n == 0 {
        0
    } else {
        n * (w as i32 + GLYPH_GAP) - GLYPH_GAP
    }
}

/// Draws `text` with its top-left corner at (`x`, `y`), clipped to `clip`; returns the x
/// position after the last character.
pub fn draw_text(
    fb: &mut Framebuffer,
    font: &Font,
    text: &str,
    (x, y): (i32, i32),
    color: TextColor,
    clip: Rect,
) -> i32 {
    let (w, h) = font.kind.size();
    let mut cx = x;
    let mut recolored = vec![0u8; w * h];
    for c in text.chars() {
        if let Some(glyph) = font.glyph(c) {
            let pixels = match color {
                TextColor::Baked => glyph,
                TextColor::Solid(ink) => {
                    for (o, &g) in recolored.iter_mut().zip(glyph) {
                        *o = if g == 0 { 0 } else { ink };
                    }
                    &recolored
                }
            };
            fb.blit(Image::new(pixels, w, h), cx, y, clip, Some(0));
        }
        cx += w as i32 + GLYPH_GAP;
    }
    cx
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_formats::font::FontKind;

    #[test]
    fn draws_and_measures() {
        let mut data = vec![0u8; FontKind::Title.file_len()];
        data[0] = 5; // 'a' glyph, top-left pixel
        let font = Font::parse(&data, FontKind::Title).unwrap();
        assert_eq!(text_width(&font, "ab"), 17);
        let mut fb = Framebuffer::new();
        let end = draw_text(
            &mut fb,
            &font,
            "a a",
            (10, 20),
            TextColor::Baked,
            Rect::SCREEN,
        );
        assert_eq!(end, 10 + 3 * 9);
        assert_eq!(fb.get(10, 20), Some(5));
        assert_eq!(fb.get(28, 20), Some(5), "third char starts at 10 + 2 * 9");
        draw_text(
            &mut fb,
            &font,
            "a",
            (0, 0),
            TextColor::Solid(9),
            Rect::SCREEN,
        );
        assert_eq!(fb.get(0, 0), Some(9));
    }
}
