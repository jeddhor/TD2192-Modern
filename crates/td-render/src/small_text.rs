//! Small-font text (`td.lt2`): dialogue lines, speaker names, captions, prompts, menus and HUD
//! messages.
//!
//! Spec: `docs/spec/render/text.md` (rev 1) §3.1–§3.3, §4.1–§4.3 and §5. Fidelity: T0
//! (CONFIRMED; the dialogue frames of oracle capture C-0002 match pixel for pixel).
//!
//! * Positions are **pen** positions, as in the original: glyph pixel (col, row) of a character
//!   drawn at pen (x, y) lands on (x + 1 + col, y + 1 + row). A string starts one pixel right of
//!   its pen, so the first inked column of a string drawn at x is x + 2 for a normal letter.
//! * Every glyph is recoloured: non-zero font bytes are drawn in the ink colour (a palette index).
//! * The advance is a fixed table (§3.2); `. , " ? - !` are drawn by code (§3.3).
//! * Characters outside the glyph table (upper case, `:`, `'` ...) advance 0 and draw no glyph.
//!   The original reads their "glyph" from memory before the font block (U-061); the port
//!   policy (§5 item 1) draws nothing, which matches every capture so far.
//! * Text is a sequence of single-byte characters: data bytes map one-to-one to `char`s.
//! * Pixels outside the 320x200 screen are dropped (port policy for the original's 352-pixel
//!   VGA rows, §5 item 3).

use td_formats::font::{Font, FontKind};

use crate::framebuffer::{Canvas, Framebuffer};

/// Glyph cell edge in pixels. Spec: text.md §3; original `file0001:051A:088F`.
pub const CELL: i32 = 5;

/// The character whose glyph (27, a solid block) is the typing cursor (text.md §3.1).
pub const CURSOR: char = '|';

/// Frames (70.086 Hz vertical retraces) to wait after each typed character.
///
/// Spec: text.md §4.3: after each character the original waits for the vertical-retrace bit,
/// which takes at most one frame and possibly none. ASSUMPTION(U-062): exactly one frame per
/// character. Oracle C-0002 agrees at preset P1: 110 characters appear in the 110 frames
/// between the key press at frame 2463 and the dump at frame 2573 (see
/// `td-flow/tests/dialogue_parity.rs`).
pub const FRAMES_PER_TYPED_CHAR: u32 = 1;

/// How a glyph's empty pixels are treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Only ink pixels are written.
    Transparent,
    /// Every pixel of the 5x5 cell is written; empty ones in `background`. In the original this
    /// is byte 650 of the font block, which every opaque caller sets to 0 (black) first.
    Opaque { background: u8 },
}

/// How far the pen moves right after drawing `ch` (text.md §3.2). Characters outside the table
/// advance 0.
pub fn advance(ch: char) -> i32 {
    match ch {
        '.' | '!' => 2,
        ',' | '1' | '"' => 3,
        'a' | 'b' | 'c' | 'e' | 'f' | 'i' | 'l' | 'p' | 'r' | 's' | 't' | 'v' | '?' | '-' | ' ' => {
            4
        }
        'd' | 'g' | 'h' | 'j' | 'k' | 'n' | 'o' | 'q' | 'u' | 'x' | 'y' | 'z' | '2'..='9' => 5,
        '0' | 'm' | 'w' => 6,
        _ => 0,
    }
}

/// Pixels (column, row) of the code-drawn marks (text.md §3.3).
fn marks(ch: char) -> &'static [(i32, i32)] {
    match ch {
        '.' => &[(0, 4)],
        ',' => &[(0, 4), (1, 3)],
        '"' => &[(1, 0), (0, 1)],
        '?' => &[(0, 0), (1, 0), (2, 1), (1, 2), (1, 4)],
        '-' => &[(0, 2), (1, 2), (2, 2)],
        '!' => &[(0, 0), (0, 1), (0, 2), (0, 4)],
        _ => &[],
    }
}

/// Draws one character with its cell origin at pen (`x`, `y`) (text.md §4.1).
pub fn draw_glyph(
    fb: &mut impl Canvas,
    font: &Font,
    ch: char,
    (x, y): (i32, i32),
    ink: u8,
    mode: Mode,
) {
    debug_assert_eq!(font.kind, FontKind::Small, "small text uses td.lt2");
    // Out-of-table characters: port policy, no glyph pixels (U-061).
    let glyph = font.glyph(ch);
    // Characters whose glyph bytes never ink, and those whose whole cell an opaque draw clears.
    let no_glyph_ink = matches!(ch, ' ' | '.' | '"' | '?' | ',' | '-');
    let clears_cell = matches!(ch, ' ' | ',' | '.' | '"' | '!' | '?' | '-');
    let background = match mode {
        Mode::Opaque { background } => Some(background),
        Mode::Transparent => None,
    };
    for row in 0..CELL {
        for col in 0..CELL {
            let p = glyph.map_or(0, |g| g[(row * CELL + col) as usize]);
            let (px, py) = (x + 1 + col, y + 1 + row);
            if p != 0 && !no_glyph_ink {
                fb.put(px, py, ink);
            } else if let Some(bg) = background {
                fb.put(px, py, bg);
            }
            if let Some(bg) = background
                && clears_cell
            {
                fb.put(px, py, bg);
            }
        }
    }
    if let Some(bg) = background
        && matches!(ch, 'm' | 'w')
    {
        // The column right of the 5-wide cell (m and w advance 6).
        for row in 1..=CELL {
            fb.put(x + 6, y + row, bg);
        }
    }
    for &(col, row) in marks(ch) {
        fb.put(x + 1 + col, y + 1 + row, ink);
    }
}

/// Draws `text` at pen (`x`, `y`) in `mode` (text.md §4.2); returns the final pen offset from
/// `x` (1 + the advances, minus one for every `x` and `y`).
pub fn draw_with(
    fb: &mut Framebuffer,
    font: &Font,
    text: &str,
    (x, y): (i32, i32),
    ink: u8,
    mode: Mode,
) -> i32 {
    let mut offset = 1;
    for ch in text.chars() {
        offset = draw_step(fb, font, ch, (x, y), offset, ink, mode);
    }
    offset
}

/// One character of a string: the x/y shift, the glyph and the advance (text.md §4.2).
fn draw_step(
    fb: &mut Framebuffer,
    font: &Font,
    ch: char,
    (x, y): (i32, i32),
    mut offset: i32,
    ink: u8,
    mode: Mode,
) -> i32 {
    if matches!(ch, 'x' | 'y') {
        offset -= 1;
    }
    draw_glyph(fb, font, ch, (x + offset, y), ink, mode);
    offset + advance(ch)
}

/// Draws `text` transparently at pen (`x`, `y`) (`Txt_DrawString`, text.md §4.2).
pub fn draw(fb: &mut Framebuffer, font: &Font, text: &str, pen: (i32, i32), ink: u8) -> i32 {
    draw_with(fb, font, text, pen, ink, Mode::Transparent)
}

/// Draws `text` opaquely at pen (`x`, `y`) (text.md §4.2, opaque variant).
pub fn draw_opaque(
    fb: &mut Framebuffer,
    font: &Font,
    text: &str,
    pen: (i32, i32),
    ink: u8,
    background: u8,
) -> i32 {
    draw_with(fb, font, text, pen, ink, Mode::Opaque { background })
}

/// Typed text (`Txt_TypeString`, text.md §4.3): an opaque string revealed one character per
/// step, each followed by the cursor, or after the last character by a space that erases it.
///
/// The type does not wait: after every step the caller waits [`FRAMES_PER_TYPED_CHAR`] frames
/// (the value [`Typewriter::step`] returns) before the next one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typewriter {
    chars: Vec<char>,
    next: usize,
    pen: (i32, i32),
    offset: i32,
    ink: u8,
    background: u8,
}

impl Typewriter {
    /// Prepares `text` for typing at pen `pen` in `ink` over `background`.
    #[must_use]
    pub fn new(text: &str, pen: (i32, i32), ink: u8, background: u8) -> Self {
        Self {
            chars: text.chars().collect(),
            next: 0,
            pen,
            offset: 1,
            ink,
            background,
        }
    }

    /// Types the next character and the cursor (or the closing space). Returns the frames to
    /// wait before the next step, or `None` (drawing nothing) once every character is typed.
    pub fn step(&mut self, fb: &mut Framebuffer, font: &Font) -> Option<u32> {
        let &ch = self.chars.get(self.next)?;
        let mode = Mode::Opaque {
            background: self.background,
        };
        self.offset = draw_step(fb, font, ch, self.pen, self.offset, self.ink, mode);
        self.next += 1;
        let after = if self.finished() { ' ' } else { CURSOR };
        let at = (self.pen.0 + self.offset, self.pen.1);
        draw_glyph(fb, font, after, at, self.ink, mode);
        Some(FRAMES_PER_TYPED_CHAR)
    }

    /// True once every character has been typed.
    pub fn finished(&self) -> bool {
        self.next >= self.chars.len()
    }

    /// Characters typed so far.
    pub fn typed(&self) -> usize {
        self.next
    }

    /// Types the rest of the string at once (no pacing).
    pub fn finish(&mut self, fb: &mut Framebuffer, font: &Font) {
        while self.step(fb, font).is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A font whose glyphs are solid 5x5 blocks except `x`, whose first column is empty (as in
    /// the real font), and `a`, which inks only its top-left pixel.
    fn test_font() -> Font {
        let mut data = vec![0xF7u8; FontKind::Small.file_len()];
        let x = 23 * 25;
        for row in 0..5 {
            data[x + row * 5] = 0;
        }
        data[1..25].fill(0); // 'a'
        Font::parse(&data, FontKind::Small).unwrap()
    }

    fn inked(fb: &Framebuffer, ink: u8) -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        for y in 0..crate::HEIGHT as i32 {
            for x in 0..crate::WIDTH as i32 {
                if fb.get(x, y) == Some(ink) {
                    v.push((x, y));
                }
            }
        }
        v
    }

    #[test]
    fn advance_table() {
        // text.md §3.2, every row.
        for (adv, chars) in [
            (2, ".!"),
            (3, ",1\""),
            (4, "abcefilprstv?- "),
            (5, "dghjknoquxyz23456789"),
            (6, "0mw"),
            (0, "AZ:'()/{|]"),
        ] {
            for c in chars.chars() {
                assert_eq!(advance(c), adv, "{c:?}");
            }
        }
    }

    #[test]
    fn strings_start_one_pixel_right_of_the_pen() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        // 'a' inks its top-left pixel only: pen 10 + offset 1 + cell 1 = column 12.
        let end = draw(&mut fb, &f, "a", (10, 20), 9);
        assert_eq!(end, 1 + 4);
        assert_eq!(inked(&fb, 9), [(12, 21)]);
        // Two leading spaces (4 each): the 'a' moves 8 to the right (text.md §4.2 example).
        let mut fb = Framebuffer::new();
        draw(&mut fb, &f, "  a", (2, 175), 9);
        assert_eq!(inked(&fb, 9), [(12, 176)]);
    }

    #[test]
    fn x_and_y_are_shifted_one_pixel_left() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        let end = draw(&mut fb, &f, "ax", (0, 0), 9);
        // 'a' at offset 1, advance 4 -> 5, 'x' drawn at offset 4 (cell columns 5..9, first
        // column empty), then +5.
        assert_eq!(end, 9);
        let cols: std::collections::BTreeSet<i32> = inked(&fb, 9).iter().map(|p| p.0).collect();
        assert_eq!(cols.into_iter().collect::<Vec<_>>(), [2, 6, 7, 8, 9]);
    }

    #[test]
    fn code_drawn_punctuation() {
        let f = test_font();
        for (ch, pixels) in [
            ('.', &[(0, 4)][..]),
            (',', &[(0, 4), (1, 3)]),
            ('"', &[(1, 0), (0, 1)]),
            ('?', &[(0, 0), (1, 0), (2, 1), (1, 2), (1, 4)]),
            ('-', &[(0, 2), (1, 2), (2, 2)]),
            ('!', &[(0, 0), (0, 1), (0, 2), (0, 4)]),
            (' ', &[]),
        ] {
            let mut fb = Framebuffer::new();
            draw_glyph(&mut fb, &f, ch, (0, 0), 9, Mode::Transparent);
            let mut want: Vec<(i32, i32)> = pixels.iter().map(|&(c, r)| (1 + c, 1 + r)).collect();
            want.sort_by_key(|&(x, y)| (y, x));
            assert_eq!(inked(&fb, 9), want, "{ch:?}");
        }
    }

    #[test]
    fn unsupported_characters_draw_nothing_and_do_not_advance() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        let end = draw(&mut fb, &f, "A:'", (0, 0), 9);
        assert_eq!(end, 1);
        assert!(inked(&fb, 9).is_empty());
        // Opaque, they clear their cell like any glyph with no ink.
        let mut fb = Framebuffer::new();
        fb.clear(7);
        draw_opaque(&mut fb, &f, "A", (0, 0), 9, 0);
        assert_eq!(fb.get(2, 1), Some(0));
        assert_eq!(fb.get(6, 5), Some(0));
        assert_eq!(fb.get(7, 5), Some(7));
    }

    #[test]
    fn opaque_cells_and_the_m_w_column() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        fb.clear(7);
        draw_glyph(&mut fb, &f, 'm', (0, 0), 9, Mode::Opaque { background: 0 });
        assert_eq!(fb.get(1, 1), Some(9));
        assert_eq!(fb.get(6, 1), Some(0), "column x + 6 cleared");
        assert_eq!(fb.get(6, 5), Some(0));
        assert_eq!(fb.get(6, 0), Some(7), "row y is outside the cell");
        assert_eq!(fb.get(7, 1), Some(7));
        // '!' clears its cell, then draws its marks.
        draw_glyph(&mut fb, &f, '!', (0, 0), 9, Mode::Opaque { background: 0 });
        assert_eq!(fb.get(1, 1), Some(9));
        assert_eq!(fb.get(1, 4), Some(0));
        assert_eq!(fb.get(2, 1), Some(0));
    }

    #[test]
    fn typewriter_draws_cursor_then_erases_it() {
        let f = test_font();
        let mut fb = Framebuffer::new();
        let mut t = Typewriter::new("ab", (0, 0), 9, 0);
        assert_eq!(t.step(&mut fb, &f), Some(FRAMES_PER_TYPED_CHAR));
        // 'a' at offset 1 (top-left pixel at column 2); cursor block at pen 5: columns 6..10.
        assert_eq!(fb.get(2, 1), Some(9));
        assert_eq!(fb.get(3, 1), Some(0));
        assert!((6..=10).all(|x| (1..=5).all(|y| fb.get(x, y) == Some(9))));
        assert!(!t.finished());
        assert_eq!(t.step(&mut fb, &f), Some(FRAMES_PER_TYPED_CHAR));
        // 'b' (solid here) over the old cursor at columns 6..10, advance 4; the closing space
        // at pen 9 clears columns 10..14.
        assert!((6..=9).all(|x| fb.get(x, 3) == Some(9)));
        assert!((10..=14).all(|x| fb.get(x, 3) == Some(0)));
        assert!(t.finished());
        assert_eq!(t.typed(), 2);
        assert_eq!(t.step(&mut fb, &f), None);
    }
}
