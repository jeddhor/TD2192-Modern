//! Drawing for the dialogue screen: the two portraits and their palette split, the speaker
//! highlight, name labels, the scrolling text area and the scene-1 prompt.
//!
//! Spec: `docs/spec/narrative/dialogue-player.md` (rev 1) §3.2, §3.4, §4.4–§4.6 and §4.8;
//! `docs/spec/render/text.md` (rev 1) §4.5. Fidelity: T0 (CONFIRMED against oracle C-0002
//! frames 2283, 2433 and 2573).
//!
//! The scene logic (reading the script, allocating text colours, pacing, fades, keys) lives in
//! `td-flow::dialogue`. Two palettes are involved, as in the original: the program's palette
//! buffer (`g_Palette`), which always holds full-brightness colours, and the DAC (the
//! [`Framebuffer`]'s palette), which shows them, possibly dimmed or faded.

use td_formats::font::Font;
use td_formats::palette::{Palette, Rgb6};
use td_formats::portraits::{FACE_HEIGHT, FACE_WIDTH};

use crate::framebuffer::{Framebuffer, WIDTH};
use crate::small_text;

/// Palette entries per face: the left face uses 0..111, the right face 112..223.
/// Spec: dialogue-player.md §3.4; original `file0001:051A:1618`.
pub const FACE_COLORS: usize = 112;
/// Largest pixel value kept from a portrait; larger values become 0. Spec: §3.4.
pub const MAX_FACE_PIXEL: u8 = 111;
/// Pen y of the name labels (ink copy). Spec: §3.2.
pub const NAME_PEN_Y: i32 = 89;
/// Pen of every newly typed line. Spec: §3.2, §3.4; original `051A:1BF7`.
pub const TEXT_PEN: (i32, i32) = (2, 175);
/// Single-row scroll steps after each line. Spec: §3.4.
pub const LINE_PITCH: usize = 7;
/// First and last rows of the text area. Row 100 is never touched. Spec: §3.2, §4.5.
pub const TEXT_ROWS: (usize, usize) = (101, 189);
/// Pen of the scene-1 prompt. Spec: §3.2; text.md §4.5.
pub const PROMPT_PEN: (i32, i32) = (1, 191);
/// Palette entry of the prompt colour. Spec: §3.2, §4.8.
pub const PROMPT_ENTRY: u8 = 254;
/// Prompt colour. Spec: §3.2.
pub const PROMPT_RGB: Rgb6 = Rgb6 { r: 32, g: 0, b: 0 };

/// A portrait half of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Slot 1: x 0..159, palette entries 0..111.
    Left,
    /// Slot 2: x 160..319, palette entries 112..223.
    Right,
}

impl Side {
    /// The side of a speaker slot: slot 1 is left, anything else is right (§4.4).
    pub fn of_slot(slot: u8) -> Side {
        if slot == 1 { Side::Left } else { Side::Right }
    }

    /// The other side.
    pub fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    /// 0 for the left side, 1 for the right.
    pub fn index(self) -> usize {
        match self {
            Side::Left => 0,
            Side::Right => 1,
        }
    }

    /// First palette entry of this side's face colours.
    pub fn first_entry(self) -> usize {
        self.index() * FACE_COLORS
    }

    /// Leftmost screen column of this side.
    pub fn x(self) -> usize {
        self.index() * FACE_WIDTH
    }
}

/// Blits a 160x100 portrait to its half of rows 0..99: values above 111 become 0, and the
/// right side adds 112 to every pixel (§4.4 steps 3 and 5).
///
/// # Panics
/// If `pixels` is not 160x100.
pub fn draw_portrait(fb: &mut Framebuffer, side: Side, pixels: &[u8]) {
    assert_eq!(pixels.len(), FACE_WIDTH * FACE_HEIGHT, "portrait size");
    let shift = side.first_entry() as u8;
    for (y, row) in pixels.chunks_exact(FACE_WIDTH).enumerate() {
        let dst = &mut fb.pixels[y * WIDTH + side.x()..][..FACE_WIDTH];
        for (d, &p) in dst.iter_mut().zip(row) {
            let kept = if p > MAX_FACE_PIXEL { 0 } else { p };
            *d = kept + shift;
        }
    }
}

/// Copies entries 0..111 of a face's palette into this side's entries of `pal` (§4.4 step 2).
pub fn load_face_palette(pal: &mut Palette, side: Side, face: &Palette) {
    let base = side.first_entry();
    pal.colors[base..base + FACE_COLORS].copy_from_slice(&face.colors[..FACE_COLORS]);
}

/// Half brightness: each 6-bit component shifted right by 1 (§3.4).
pub fn half(c: Rgb6) -> Rgb6 {
    Rgb6 {
        r: c.r >> 1,
        g: c.g >> 1,
        b: c.b >> 1,
    }
}

/// Writes DAC entries 0..223 from `pal` at half brightness: both faces dim (a speaker defined
/// while the screen is lit, §4.4 step 5.4).
pub fn dim_both_faces(dac: &mut Palette, pal: &Palette) {
    for i in 0..2 * FACE_COLORS {
        dac.colors[i] = half(pal.colors[i]);
    }
}

/// The highlight before each line (§4.5 step 3): the speaker's face entries at full
/// brightness, the other face's at half. Left face entries 1..111 and right face entries
/// 112..223; entry 0 is not touched.
pub fn highlight(dac: &mut Palette, pal: &Palette, speaker: Side) {
    for side in [Side::Left, Side::Right] {
        let first = side.first_entry().max(1);
        for i in first..side.first_entry() + FACE_COLORS {
            let c = pal.colors[i];
            dac.colors[i] = if side == speaker { c } else { half(c) };
        }
    }
}

/// Pen of a name label's ink copy: x0 + 1 with x0 = side × 160 + 79 − (length div 2) × 5
/// (§3.2; text.md §4.5).
pub fn name_pen(side: Side, name: &str) -> (i32, i32) {
    let len = name.chars().count() as i32;
    let x0 = side.x() as i32 + 79 - (len / 2) * 5;
    (x0 + 1, NAME_PEN_Y)
}

/// Draws a name label: black copies at (x0, 90), (x0, 89) and (x0 + 1, 90), then the name in
/// `ink` at (x0 + 1, 89) (§3.2). Transparent small text.
pub fn draw_name(fb: &mut Framebuffer, font: &Font, side: Side, name: &str, ink: u8) {
    let (x, y) = name_pen(side, name);
    for pen in [(x - 1, y + 1), (x - 1, y), (x, y + 1)] {
        small_text::draw(fb, font, name, pen, 0);
    }
    small_text::draw(fb, font, name, (x, y), ink);
}

/// Scrolls the text area up one line: 7 times, rows 102..189 are copied one row up, so row 189
/// is duplicated and row 100 never changes (§4.5 step 5).
pub fn scroll_text_area(fb: &mut Framebuffer) {
    scroll_text_rows(fb, TEXT_ROWS.1);
}

/// As [`scroll_text_area`] with the area ending at row `bottom` (E2 and E3: 199,
/// episode-deltas rev 2 §9.3).
pub fn scroll_text_rows(fb: &mut Framebuffer, bottom: usize) {
    let top = TEXT_ROWS.0;
    for _ in 0..LINE_PITCH {
        fb.pixels
            .copy_within((top + 1) * WIDTH..(bottom + 1) * WIDTH, top * WIDTH);
    }
}

/// Draws the scene-1 prompt: black copies at pens (0, 191), (1, 191) and (2, 191), then the
/// text in entry 254 at (1, 191) (§3.2; text.md §4.5). The caller sets entry 254 to
/// [`PROMPT_RGB`] in its palette buffer.
pub fn draw_prompt(fb: &mut Framebuffer, font: &Font, text: &str) {
    let (x, y) = PROMPT_PEN;
    for dx in [-1, 0, 1] {
        small_text::draw(fb, font, text, (x + dx, y), 0);
    }
    small_text::draw(fb, font, text, (x, y), PROMPT_ENTRY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portraits_are_clamped_and_offset() {
        let mut pixels = vec![0u8; FACE_WIDTH * FACE_HEIGHT];
        pixels[0] = 5;
        pixels[1] = 111;
        pixels[2] = 112;
        pixels[3] = 147;
        let mut fb = Framebuffer::new();
        draw_portrait(&mut fb, Side::Right, &pixels);
        let row: Vec<_> = (160..165).map(|x| fb.get(x, 0).unwrap()).collect();
        assert_eq!(
            row,
            [117, 223, 112, 112, 112],
            "values > 111 become 0, then + 112"
        );
        draw_portrait(&mut fb, Side::Left, &pixels);
        let row: Vec<_> = (0..5).map(|x| fb.get(x, 0).unwrap()).collect();
        assert_eq!(row, [5, 111, 0, 0, 0]);
        assert_eq!(fb.get(0, 100), Some(0), "row 100 untouched");
    }

    #[test]
    fn palette_split_and_highlight() {
        let mut face = Palette::default();
        face.colors[0] = Rgb6 { r: 2, g: 2, b: 2 };
        face.colors[5] = Rgb6 { r: 63, g: 31, b: 2 };
        face.colors[112] = Rgb6 { r: 9, g: 9, b: 9 };
        let mut pal = Palette::default();
        load_face_palette(&mut pal, Side::Left, &face);
        load_face_palette(&mut pal, Side::Right, &face);
        assert_eq!(pal.colors[117], face.colors[5]);
        assert_eq!(
            pal.colors[224],
            Rgb6::default(),
            "only entries 0..111 are copied"
        );
        let mut dac = Palette::default();
        dac.colors[0] = Rgb6 { r: 7, g: 7, b: 7 };
        highlight(&mut dac, &pal, Side::Right);
        assert_eq!(
            dac.colors[0],
            Rgb6 { r: 7, g: 7, b: 7 },
            "entry 0 untouched"
        );
        assert_eq!(dac.colors[5], Rgb6 { r: 31, g: 15, b: 1 });
        assert_eq!(
            dac.colors[112], face.colors[0],
            "right entry 112 is highlighted"
        );
        assert_eq!(dac.colors[117], face.colors[5]);
        dim_both_faces(&mut dac, &pal);
        assert_eq!(dac.colors[0], Rgb6 { r: 1, g: 1, b: 1 });
        assert_eq!(dac.colors[117], Rgb6 { r: 31, g: 15, b: 1 });
    }

    #[test]
    fn name_positions() {
        // text.md §6 vector 2: 13 characters on the left, 10 on the right.
        assert_eq!(name_pen(Side::Left, "abcdefghijklm"), (50, 89));
        assert_eq!(name_pen(Side::Right, "abcdefghij"), (215, 89));
    }

    #[test]
    fn scrolling_moves_the_text_area_by_seven_rows() {
        let mut fb = Framebuffer::new();
        for y in 99..=191 {
            fb.put(0, y, y as u8);
        }
        scroll_text_area(&mut fb);
        assert_eq!(fb.get(0, 100), Some(100), "row 100 never changes");
        assert_eq!(fb.get(0, 101), Some(108));
        assert_eq!(fb.get(0, 182), Some(189));
        assert_eq!(fb.get(0, 183), Some(189), "row 189 is duplicated");
        assert_eq!(fb.get(0, 189), Some(189));
        assert_eq!(fb.get(0, 190), Some(190), "below the area");
    }
}
