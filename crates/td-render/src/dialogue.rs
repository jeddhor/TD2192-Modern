//! The static parts of the dialogue screen: two portraits, their palettes, speaker colours and
//! name labels.
//!
//! Observed in the original (oracle C-0002 dialogue frames; `docs/re/dynamic-observations.md`
//! §12). T0 for these parts:
//! * slot 1's portrait is drawn at (0, 0) with its own indices, and its palette entries 0..112
//!   are loaded into entries 0..112;
//! * slot 2's portrait is drawn at (160, 0) with every index + 112 (background 0 becomes 112),
//!   and its entries 0..112 go into 112..224;
//! * the slot that is not speaking has its portrait palette halved (`v >> 1`);
//! * speaker colours: `td.pal[colour byte]` is loaded into entry 249 + slot;
//! * names are drawn in the speaker colour at y = 90, centred on x = 80 / 240 as
//!   `centre - (width + 4) / 2` (LIKELY: two observed names), over a black drop shadow.
//!
//! The text area (line reveal, scrolling, page breaks) is not covered here yet (RE pending).

use td_formats::font::Font;
use td_formats::palette::{Palette, Rgb6};
use td_formats::portraits::{FACE_HEIGHT, FACE_WIDTH, Portrait};

use crate::framebuffer::{Framebuffer, Rect};
use crate::small_text;

/// Palette entries per portrait (face entries 0..112). Some E2 faces use pixel indices up to
/// 147; how those display is unverified (U-019).
pub const FACE_COLORS: usize = 112;
/// Palette entry for slot `s`'s text colour is `SPEAKER_COLOR_BASE + s`.
pub const SPEAKER_COLOR_BASE: u8 = 249;
/// Name labels' top row.
pub const NAME_Y: i32 = 90;

/// Horizontal origin of a slot's portrait (slot 1 or 2).
fn portrait_x(slot: u8) -> usize {
    if slot == 2 { FACE_WIDTH } else { 0 }
}

/// Draws a slot's portrait and loads its palette, dimmed when the slot is not speaking.
pub fn draw_portrait(fb: &mut Framebuffer, slot: u8, face: &Portrait, speaking: bool) {
    let offset = if slot == 2 { FACE_COLORS as u8 } else { 0 };
    let x0 = portrait_x(slot);
    for y in 0..FACE_HEIGHT {
        for x in 0..FACE_WIDTH {
            let p = face.pixels[y * FACE_WIDTH + x];
            fb.pixels[y * crate::WIDTH + x0 + x] = p.wrapping_add(offset);
        }
    }
    load_portrait_palette(&mut fb.palette, slot, &face.palette, speaking);
}

/// Loads (or re-dims) a slot's portrait palette without redrawing pixels.
pub fn load_portrait_palette(pal: &mut Palette, slot: u8, face: &Palette, speaking: bool) {
    let offset = if slot == 2 { FACE_COLORS } else { 0 };
    for i in 0..FACE_COLORS {
        let c = face.colors[i];
        pal.colors[i + offset] = if speaking {
            c
        } else {
            Rgb6 {
                r: c.r >> 1,
                g: c.g >> 1,
                b: c.b >> 1,
            }
        };
    }
}

/// Sets a slot's text colour from the flight palette and the speaker's colour byte.
pub fn set_speaker_color(pal: &mut Palette, slot: u8, td_pal: &Palette, color_byte: u8) {
    pal.colors[(SPEAKER_COLOR_BASE + slot) as usize] = td_pal.colors[color_byte as usize];
}

/// Draws a slot's name label, centred under its portrait, with a black drop shadow (each glyph
/// pixel also blackens the pixels to its left, below, and below-left).
pub fn draw_name(fb: &mut Framebuffer, font: &Font, slot: u8, name: &str) {
    let centre = portrait_x(slot) as i32 + FACE_WIDTH as i32 / 2;
    let x = centre - (small_text::width(font, name) + 4) / 2;
    for (dx, dy) in [(-1, 0), (0, 1), (-1, 1)] {
        small_text::draw(fb, font, name, (x + dx, NAME_Y + dy), 0, Rect::SCREEN);
    }
    small_text::draw(
        fb,
        font,
        name,
        (x, NAME_Y),
        SPEAKER_COLOR_BASE + slot,
        Rect::SCREEN,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_portrait_is_offset_and_listener_dimmed() {
        let mut pixels = vec![0u8; FACE_WIDTH * FACE_HEIGHT];
        pixels[0] = 5;
        let mut palette = Palette::default();
        palette.colors[5] = Rgb6 { r: 63, g: 31, b: 2 };
        let face = Portrait {
            palette,
            pixels: pixels.into_boxed_slice(),
        };
        let mut fb = Framebuffer::new();
        draw_portrait(&mut fb, 2, &face, false);
        assert_eq!(fb.get(160, 0), Some(117));
        assert_eq!(fb.get(161, 0), Some(112), "background is offset too");
        assert_eq!(fb.palette.colors[117], Rgb6 { r: 31, g: 15, b: 1 });
        draw_portrait(&mut fb, 1, &face, true);
        assert_eq!(fb.get(0, 0), Some(5));
        assert_eq!(fb.palette.colors[5], Rgb6 { r: 63, g: 31, b: 2 });
    }
}
