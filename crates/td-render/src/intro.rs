//! Drawing for the intro program (`file0000.exe`): its two small-font string routines, the
//! 64-level full fade, and the flyover's tile grid, hoverskid blit and captions, all in
//! [`VgaMemory`].
//!
//! Spec: `docs/spec/flow/intro.md` (rev 1) §2, §3.1, §4.2, §4.4–§4.6; `docs/spec/render/text.md`
//! (rev 2) §3–§4 for the glyphs and advances. Fidelity: T0 (C-0047's frames match, see
//! `td-flow/tests/intro_parity.rs`).
//!
//! Original bugs kept: the hoverskid blit flips the sprite vertically and reads one column to the
//! right (OB-059, preserved in every profile), and it uses only the low byte of its y (OB-058,
//! preserved in Classic; [`Hoverskid::Clipped`] is the Enhanced fix).

use td_formats::font::Font;
use td_formats::palette::{Palette, Rgb6};

use crate::framebuffer::Canvas;
use crate::small_text::{self, Mode};
use crate::vga::{ROW_BYTES, VgaMemory, row_offset};

/// Levels of the intro's full fades: 0..=64, one per retrace (intro §4.2).
pub const FADE_LEVELS: u8 = 65;

/// One level of a full fade (`0000:0016` in, `0000:00AF` out): DAC entries 1..=255 become
/// `working × level div 64` (truncating); entry 0 is never written (intro §4.2).
///
/// # Panics
/// If `level` > 64.
pub fn full_fade_level(dac: &mut Palette, working: &Palette, level: u8) {
    assert!(level < FADE_LEVELS, "fade level {level} out of range");
    let scale = |v: u8| ((u16::from(v) * u16::from(level)) / 64) as u8;
    for (d, s) in dac.colors.iter_mut().zip(&working.colors).skip(1) {
        *d = Rgb6 {
            r: scale(s.r),
            g: scale(s.g),
            b: scale(s.b),
        };
    }
}

/// The intro's two string routines (intro §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    /// `Text B` (`0000:06BC`): text.md's spacing exactly.
    B,
    /// `Text A` (`0000:0302`): one extra pixel before every character but `x` and `y`.
    A,
}

/// How far the intro's pen moves after `ch`: text.md §3.2, plus `:` = 2 and `^` = `2`'s 5.
#[must_use]
pub fn advance(ch: char) -> i32 {
    match ch {
        ':' => 2,
        '^' => small_text::advance('2'),
        _ => small_text::advance(ch),
    }
}

/// Draws one character at pen (`x`, `y`), transparent (intro §4.4): `:` is two code-drawn
/// pixels, `^` is a `2` four rows higher, everything else as text.md §4.1.
fn draw_char(canvas: &mut impl Canvas, font: &Font, ch: char, (x, y): (i32, i32), ink: u8) {
    match ch {
        ':' => {
            canvas.put(x + 1, y + 2, ink);
            canvas.put(x + 1, y + 4, ink);
        }
        '^' => small_text::draw_glyph(canvas, font, '2', (x, y - 4), ink, Mode::Transparent),
        _ => small_text::draw_glyph(canvas, font, ch, (x, y), ink, Mode::Transparent),
    }
}

/// Draws `text` at pen (`x`, `y`) with routine `style`, transparent; returns the final pen
/// offset from `x` (intro §4.4).
pub fn draw_text(
    canvas: &mut impl Canvas,
    font: &Font,
    style: TextStyle,
    text: &str,
    (x, y): (i32, i32),
    ink: u8,
) -> i32 {
    let mut offset = 1;
    for ch in text.chars() {
        let xy = matches!(ch, 'x' | 'y');
        match style {
            TextStyle::B if xy => offset -= 1,
            TextStyle::A if !xy => offset += 1,
            _ => {}
        }
        draw_char(canvas, font, ch, (x + offset, y), ink);
        offset += advance(ch);
    }
    offset
}

/// Rows from one flyover page's origin to the next: page p starts at VGA row 264 p (intro §2).
pub const PAGE_ROWS: u16 = 264;
/// Rows from a page's origin to its tile grid (intro §2).
pub const GRID_TOP: u16 = 32;
/// VGA byte of `td.blk`'s tile 0: row 528 (intro §3.1, §4.5).
pub const TILE_BASE: u16 = 0xB580;
/// VGA byte where `td.dun` starts: row 734, x 192, which is tile slot 71 (intro §3.1).
pub const DUN_BASE: u16 = 0xFC80;
/// Bytes of `td.blk` the intro loads: 232 rows of 352 pixels (intro §3.1).
pub const BLK_LOADED: usize = 232 * 352;
/// Columns of the flyover grid (the 11th, c = 10, lies right of the screen).
pub const GRID_COLUMNS: u8 = 11;
/// Rows of the flyover grid.
pub const GRID_ROWS: u8 = 8;
/// Map columns and rows of `intro.map` (intro §3.2).
pub const MAP_COLUMNS: usize = 20;
/// Rows of `intro.map`.
pub const MAP_ROWS: usize = 2000;

/// Byte offset of page `page`'s origin.
#[must_use]
pub fn page_origin(page: u8) -> u16 {
    row_offset(PAGE_ROWS * u16::from(page & 1))
}

/// The CRTC start that shows page `page` scrolled by `v198` (intro §4.6 step 8):
/// `0B00h + 5AC0h × page + (v198 − 16) × 88`.
#[must_use]
pub fn page_start(page: u8, v198: i16) -> u16 {
    page_origin(page)
        .wrapping_add(row_offset(GRID_TOP))
        .wrapping_add((v198 - 16).wrapping_mul(ROW_BYTES as i16) as u16)
}

/// `intro.map` element (i, j), 1-based as the original's `array[1..20, 1..2000]`: file byte
/// `(i − 1) × 2000 + (j − 1)` (intro §3.2). Out of range reads 0.
#[must_use]
pub fn map_tile(map: &[u8], i: usize, j: usize) -> u8 {
    if !(1..=MAP_COLUMNS).contains(&i) || !(1..=MAP_ROWS).contains(&j) {
        return 0;
    }
    map.get((i - 1) * MAP_ROWS + (j - 1)).copied().unwrap_or(0)
}

/// Loads the tiles as the intro does (intro §3.1): `td.blk`'s first 232 rows from VGA row 528
/// (wrapping past the end of memory), then `td.dun` over tile slots 71..73.
pub fn load_tiles(vga: &mut VgaMemory, blk: &[u8], dun: &[u8]) {
    vga.load_linear(TILE_BASE, &blk[..blk.len().min(BLK_LOADED)]);
    vga.load_linear(DUN_BASE, dun);
}

/// One grid cell (`0957:01D5`, intro §4.5): tile `tile` copied with latched writes from VGA byte
/// `(B580h + 256 × tile) mod 65536` to page `page`'s cell (`c`, `r`) at VGA row
/// `32 + 264 page + 32 r`, x = 32 c.
pub fn draw_tile(vga: &mut VgaMemory, page: u8, c: u8, r: u8, tile: u8) {
    let src = TILE_BASE.wrapping_add(256u16.wrapping_mul(u16::from(tile)));
    let dst = page_origin(page)
        .wrapping_add(row_offset(GRID_TOP + 32 * u16::from(r)))
        .wrapping_add(8 * u16::from(c));
    vga.copy_latched(src, dst, 32, 8, 8, ROW_BYTES);
}

/// The flyover grid on `page` (intro §4.6 step 3): columns 0..`columns`, rows 0..8, tile
/// `map[c + 5, r + v194]`.
pub fn draw_grid(vga: &mut VgaMemory, map: &[u8], page: u8, v194: i16, columns: u8) {
    for c in 0..columns {
        for r in 0..GRID_ROWS {
            let j = (i32::from(r) + i32::from(v194)).max(0) as usize;
            let tile = map_tile(map, usize::from(c) + 5, j);
            draw_tile(vga, page, c, r, tile);
        }
    }
}

/// How the hoverskid blit places its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hoverskid {
    /// The original (Classic, OB-058 preserved): only the low byte of y is used, so the sprite
    /// can land 256 rows higher, in the other page.
    LowByte,
    /// The Enhanced fix: the full y, and rows outside the page (264 rows from its origin) are
    /// not drawn.
    Clipped,
}

/// The hoverskid blit (`0957:0000(page, sprite, y, x)`, intro §4.5). Destination pixel
/// (dx, dy), dx, dy = 0..31, is VGA row `232 page + (y mod 256) + dy`, x `(x and FFFCh) + dx`;
/// it takes `sheet[sprite × 1024 + (31 − dy) × 32 + dx + 1]` and is written unless 0 (OB-059:
/// flipped vertically and shifted one pixel left, the last column reading the row below).
pub fn draw_hoverskid(
    vga: &mut VgaMemory,
    sheet: &[u8],
    page: u8,
    sprite: usize,
    y: i16,
    x: i16,
    mode: Hoverskid,
) {
    let page = i32::from(page & 1);
    let base_row = match mode {
        Hoverskid::LowByte => 232 * page + i32::from(y).rem_euclid(256),
        Hoverskid::Clipped => 232 * page + i32::from(y),
    };
    let x0 = i32::from(x) & !3;
    let page_rows = 264 * page..264 * (page + 1);
    for dy in 0..32 {
        let row = base_row + dy;
        if mode == Hoverskid::Clipped && !page_rows.contains(&row) {
            continue;
        }
        for dx in 0..32 {
            let src = sprite * 1024 + (31 - dy as usize) * 32 + dx as usize + 1;
            let c = sheet.get(src).copied().unwrap_or(0);
            if c != 0 {
                vga.set_pixel(x0 + dx, row, c);
            }
        }
    }
}

/// A flyover caption (intro §4.4, §4.6 step 6): Text A in colour 0 at (cx − 1 − 6 (len div 2),
/// `row` + 1), then in `ink` at (cx − 6 (len div 2), `row`), `row` being a VGA row. `len`
/// counts every character, leading spaces included.
pub fn draw_caption(vga: &mut VgaMemory, font: &Font, text: &str, cx: i32, row: i32, ink: u8) {
    let x = caption_x(text, cx);
    draw_text(vga, font, TextStyle::A, text, (x - 1, row + 1), 0);
    draw_text(vga, font, TextStyle::A, text, (x, row), ink);
}

/// Pen x of a caption centred on `cx`: `cx − 6 × (len div 2)`.
#[must_use]
pub fn caption_x(text: &str, cx: i32) -> i32 {
    cx - 6 * (text.chars().count() as i32 / 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use td_formats::font::FontKind;

    /// Glyphs: every glyph inks only its top-left pixel.
    fn dot_font() -> Font {
        let mut data = vec![0u8; FontKind::Small.file_len()];
        for g in data.chunks_exact_mut(25) {
            g[0] = 1;
        }
        Font::parse(&data, FontKind::Small).unwrap()
    }

    fn inked(v: &VgaMemory, rows: std::ops::Range<i32>, ink: u8) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for y in rows {
            for x in 0..352 {
                if v.pixel(x, y) == ink {
                    out.push((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn full_fade_truncates() {
        let mut w = Palette::default();
        w.colors[0] = Rgb6 { r: 9, g: 9, b: 9 };
        w.colors[1] = Rgb6 { r: 63, g: 1, b: 33 };
        let mut dac = Palette::default();
        full_fade_level(&mut dac, &w, 63);
        assert_eq!(dac.colors[1], Rgb6 { r: 62, g: 0, b: 32 });
        full_fade_level(&mut dac, &w, 64);
        assert_eq!(dac.colors[1], w.colors[1]);
        assert_eq!(dac.colors[0], Rgb6::default(), "entry 0 never written");
    }

    #[test]
    fn text_b_and_text_a_spacing() {
        let f = dot_font();
        let mut v = VgaMemory::new();
        // Text B: 'a' at offset 1 (column x + 2), advance 4; 'x' moves one left: offset 4.
        let end = draw_text(&mut v, &f, TextStyle::B, "ax", (10, 300), 7);
        assert_eq!(end, 1 + 4 - 1 + 5);
        assert_eq!(inked(&v, 300..310, 7), [(12, 301), (15, 301)]);
        // Text A: 'a' at offset 2, then 'x' (no extra pixel) at offset 6.
        let mut v = VgaMemory::new();
        let end = draw_text(&mut v, &f, TextStyle::A, "ax", (10, 300), 7);
        assert_eq!(end, 2 + 4 + 5);
        assert_eq!(inked(&v, 300..310, 7), [(13, 301), (17, 301)]);
    }

    #[test]
    fn colon_and_superscript_two() {
        let f = dot_font();
        let mut v = VgaMemory::new();
        let end = draw_text(&mut v, &f, TextStyle::B, ":^", (0, 100), 7);
        assert_eq!(end, 1 + 2 + 5);
        // ':' at offset 1: pixels (0, 1) and (0, 3) of the cell; '^' = '2' four rows up.
        assert_eq!(inked(&v, 90..110, 7), [(4, 97), (2, 102), (2, 104)]);
    }

    #[test]
    fn page_starts_and_grid_rows() {
        assert_eq!(page_start(0, 16), 0x0B00);
        assert_eq!(page_start(1, 16), 0x0B00 + 0x5AC0);
        assert_eq!(page_start(0, 47), 0x0B00 + 31 * 88);
        let mut v = VgaMemory::new();
        // Tile 2's pixel (3, 1) is VGA byte B580h + 512 + 8 + 0, plane 3.
        v.set_byte(3, TILE_BASE + 512 + 8, 99);
        draw_tile(&mut v, 1, 2, 1, 2);
        // Page 1, cell (2, 1): row 32 + 264 + 32 + 1, x = 64 + 3.
        assert_eq!(v.pixel(67, 329), 99);
    }

    #[test]
    fn map_is_column_major_20_by_2000() {
        let mut map = vec![0u8; 40_000];
        map[2 * 2000 + 4] = 50; // i = 3, j = 5
        assert_eq!(map_tile(&map, 3, 5), 50);
        assert_eq!(map_tile(&map, 0, 5), 0);
        assert_eq!(map_tile(&map, 21, 1), 0);
    }

    #[test]
    fn hoverskid_is_flipped_shifted_and_wraps_on_the_low_byte() {
        let mut sheet = vec![0u8; 20 * 1024];
        sheet[4 * 1024 + 31 * 32 + 1] = 5; // record 4, row 31, column 1
        sheet[5 * 1024] = 6; // record 5, row 0, column 0
        sheet[4 * 1024] = 7; // record 4, column 0: never drawn
        let mut v = VgaMemory::new();
        draw_hoverskid(&mut v, &sheet, 0, 4, 10, 142, Hoverskid::LowByte);
        // Source row 31 column 1 -> dy 0, dx 0 at x 140.
        assert_eq!(v.pixel(140, 10), 5);
        // dx 31 on dy 0 reads byte (32 − 0) × 32 = record 5's first byte.
        assert_eq!(v.pixel(171, 10), 6);
        assert!(inked(&v, 0..600, 7).is_empty(), "column 0 never drawn");
        // Page 1 with y = 250: rows 232 + (250 mod 256) .. ; y = 260 wraps to 232 + 4.
        let mut v = VgaMemory::new();
        draw_hoverskid(&mut v, &sheet, 1, 4, 260, 142, Hoverskid::LowByte);
        assert_eq!(v.pixel(140, 236), 5);
        let mut v = VgaMemory::new();
        draw_hoverskid(&mut v, &sheet, 1, 4, 260, 142, Hoverskid::Clipped);
        assert_eq!(v.pixel(140, 236), 0, "the fix never writes the other page");
        assert_eq!(v.pixel(140, 492), 5);
        let mut v = VgaMemory::new();
        draw_hoverskid(&mut v, &sheet, 1, 4, 300, 142, Hoverskid::Clipped);
        assert!(inked(&v, 0..800, 5).is_empty(), "rows past the page are clipped");
    }

    #[test]
    fn caption_centring_counts_leading_spaces() {
        assert_eq!(caption_x("  abcd", 160), 160 - 18);
        assert_eq!(caption_x("abc", 250), 244);
    }
}
