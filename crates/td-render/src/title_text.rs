//! Title text (`td-let.cin`): mission title cards and a few headings.
//!
//! Spec: `docs/spec/render/text.md` (rev 1) §3.4, §4.4, §4.5 (title card) and §5 items 2 and 4.
//! Fidelity: T0 (CONFIRMED; the mission 1 title card of oracle capture C-0002 matches pixel for
//! pixel).
//!
//! * The string is upper-cased first (`UpCase`: only `a`..`z` change).
//! * Only font pixels equal to `F7h` are drawn, in the ink colour; everything else is
//!   transparent.
//! * The game loads only the first 39 of the 40 glyphs, so `:` (glyph 39) draws nothing (port
//!   policy for the uninitialised memory the original reads, §5 item 2). `.` and `,` are fixed
//!   shapes; space draws nothing. Other characters have no glyph and draw nothing (the spec
//!   lists no others; this follows the `:` policy).
//! * The advance and the measured width follow two different formulas, as in the original
//!   (§3.4, §5 item 4).

use td_formats::font::{Font, FontKind};

use crate::framebuffer::Framebuffer;

/// Glyph cell edge in pixels. Spec: text.md §3; original `file0001:051A:042A`.
pub const CELL: i32 = 8;
/// The only font byte value that is drawn. Spec: text.md §3.4.
pub const INK_BYTE: u8 = 0xF7;
/// Glyphs the game loads (2,496 of the file's 2,560 bytes). Spec: text.md §3.4; original
/// `file0001:051A:5852`.
pub const LOADED_GLYPHS: usize = 39;
/// Screen centre used by [`centred_x`]. Spec: text.md §4.5 (mission title card).
pub const CENTRE_X: i32 = 160;

/// `UpCase`: only `a`..`z` change.
fn upcase(ch: char) -> char {
    if ch.is_ascii_lowercase() {
        ch.to_ascii_uppercase()
    } else {
        ch
    }
}

/// Characters whose drawn advance is 6 instead of 7.
fn is_narrow(ch: char) -> bool {
    matches!(ch, 'C' | 'E' | 'Z' | 'O' | '3' | '5')
}

/// The code-drawn shapes, as (column, row) cell pixels (text.md §3.4).
fn shape(ch: char) -> Option<&'static [(i32, i32)]> {
    match ch {
        '.' => Some(&[(2, 6), (3, 6), (2, 7), (3, 7)]),
        ',' => Some(&[(2, 5), (3, 6), (2, 7), (3, 7)]),
        _ => None,
    }
}

/// Plots one upper-cased character at pen (`x`, `y`) (text.md §4.4).
fn draw_char(fb: &mut Framebuffer, font: &Font, ch: char, (x, y): (i32, i32), ink: u8) {
    if ch == ' ' {
        return;
    }
    if let Some(pixels) = shape(ch) {
        for &(col, row) in pixels {
            fb.put(x + 1 + col, y + 1 + row, ink);
        }
        return;
    }
    let Some(index) = font.kind.glyph_index(ch).filter(|&i| i < LOADED_GLYPHS) else {
        return;
    };
    let Some(glyph) = font.glyphs.get(index) else {
        return;
    };
    for row in 0..CELL {
        for col in 0..CELL {
            if glyph[(row * CELL + col) as usize] == INK_BYTE {
                fb.put(x + 1 + col, y + 1 + row, ink);
            }
        }
    }
}

/// Draws `text` transparently at pen (`x`, `y`) in `ink` (`Txt_DrawTitle`, text.md §4.4);
/// returns the pen x after the text.
pub fn draw(fb: &mut Framebuffer, font: &Font, text: &str, (x, y): (i32, i32), ink: u8) -> i32 {
    debug_assert_eq!(font.kind, FontKind::Title, "title text uses td-let.cin");
    let mut pen = x;
    for ch in text.chars().map(upcase) {
        match ch {
            '1' => pen -= 1,
            'M' | 'W' => pen += 1,
            _ => {}
        }
        draw_char(fb, font, ch, (pen, y), ink);
        pen += if is_narrow(ch) { 6 } else { 7 };
        match ch {
            '1' => pen -= 2,
            'J' => pen += 1,
            'M' | 'V' | 'W' => pen += 2,
            _ => {}
        }
    }
    pen
}

/// The measured width used for centring (`Txt_MeasureTitle`, text.md §3.4). It differs from
/// the drawn advance for some characters; the original's formula is kept as it is.
pub fn width(text: &str) -> i32 {
    text.chars()
        .map(upcase)
        .map(|ch| {
            let mut w = if ch == '1' { 6 } else { 7 };
            if ch == 'M' {
                w += 1;
            }
            if is_narrow(ch) {
                w -= 1;
            }
            match ch {
                '1' => w -= 2,
                'J' => w += 1,
                'M' | 'V' | 'W' => w += 2,
                _ => {}
            }
            w
        })
        .sum()
}

/// Pen x that centres `text` on the screen: 160 − Round(width / 2), with `Round` taking halves
/// away from zero (text.md §4.5, mission title card).
pub fn centred_x(text: &str) -> i32 {
    let w = width(text);
    // Round(w / 2) for w >= 0: halves round up.
    CENTRE_X - (w + 1).div_euclid(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glyph i has its top-left pixel set to F7h and its second pixel to another colour.
    fn test_font() -> Font {
        let mut data = vec![0u8; FontKind::Title.file_len()];
        for g in 0..40 {
            data[g * 64] = INK_BYTE;
            data[g * 64 + 1] = 0x3C;
        }
        Font::parse(&data, FontKind::Title).unwrap()
    }

    fn inked(fb: &Framebuffer) -> Vec<(i32, i32)> {
        (0..fb.pixels.len())
            .filter(|&i| fb.pixels[i] != 0)
            .map(|i| ((i % crate::WIDTH) as i32, (i / crate::WIDTH) as i32))
            .collect()
    }

    #[test]
    fn only_f7_pixels_are_drawn_after_upcase() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        let end = draw(&mut fb, &f, "a", (10, 20), 85);
        assert_eq!(end, 17);
        assert_eq!(inked(&fb), [(11, 21)]);
        assert_eq!(fb.get(11, 21), Some(85));
    }

    #[test]
    fn colon_is_not_loaded_and_space_draws_nothing() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        assert_eq!(draw(&mut fb, &f, ": ", (0, 0), 85), 14);
        assert!(inked(&fb).is_empty());
        draw(&mut fb, &f, "!", (0, 0), 85);
        assert_eq!(inked(&fb), [(1, 1)], "glyph 38 is the last one loaded");
    }

    #[test]
    fn dot_and_comma_shapes() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        draw(&mut fb, &f, ".", (0, 0), 85);
        assert_eq!(inked(&fb), [(3, 7), (4, 7), (3, 8), (4, 8)]);
        let mut fb = Framebuffer::new();
        draw(&mut fb, &f, ",", (0, 0), 85);
        assert_eq!(inked(&fb), [(3, 6), (4, 7), (3, 8), (4, 8)]);
    }

    #[test]
    fn advance_rules() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        for (text, end) in [
            ("A", 7),
            ("C", 6),
            ("1", 4),
            ("J", 8),
            ("M", 10),
            ("V", 9),
            ("W", 10),
            ("e5", 12),
            (" ", 7),
        ] {
            assert_eq!(draw(&mut fb, &f, text, (0, 100), 85), end, "{text}");
        }
        // '1' is drawn one pixel left of its pen, 'M' and 'W' one pixel right.
        let mut fb = Framebuffer::new();
        draw(&mut fb, &f, "A1M", (0, 0), 85);
        assert_eq!(inked(&fb), [(1, 1), (7, 1), (13, 1)]);
    }

    #[test]
    fn measure_rules() {
        assert_eq!(width("A"), 7);
        assert_eq!(width("1"), 4);
        assert_eq!(width("M"), 10);
        assert_eq!(
            width("W"),
            9,
            "measured W differs from its drawn advance (10)"
        );
        assert_eq!(width("J"), 8);
        assert_eq!(width("V"), 9);
        assert_eq!(width("co35z"), 30);
        assert_eq!(width(" "), 7);
        assert_eq!(centred_x("A"), 160 - 4, "Round(3.5) = 4");
        assert_eq!(centred_x("AA"), 160 - 7);
    }
}
