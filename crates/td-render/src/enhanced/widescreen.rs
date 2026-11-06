//! Widescreen composition helpers (`docs/spec/enhanced/presentation.md` rev 1 §4.3, M12-09):
//! the HUD band of a widescreen flight frame, and Classic overlays drawn into a wide frame.
//!
//! The playfield itself is drawn by [`crate::flight`], whose drawing functions take a wide
//! framebuffer as it is. Front-end screens are centred by the pipeline's palette stage.

use td_formats::palette::Rgb6;

use crate::flight::{HUD_H, HUD_W, HUD_Y, PLAYFIELD, Viewport, draw_hud_bar};
use crate::framebuffer::{Framebuffer, HEIGHT, WIDTH};
use crate::hud::Hud;

/// Columns of the bar mirrored into each side panel (§4.3).
const MIRRORED: usize = 16;

/// Draws the instruments like [`Hud::draw`]:
/// - the guidance dots go into the playfield;
/// - the bar goes into rows 160..191, at x = m in a widescreen frame, with the side panels
///   filled by [`mirror_hud_panels`];
/// - the animated entries go into the palette.
pub fn draw_hud(fb: &mut Framebuffer, hud: &Hud) {
    let vp = Viewport::of(fb);
    if vp.is_classic() {
        hud.draw(fb);
        return;
    }
    let m = vp.margin();
    for &(x, y, c) in hud.dots() {
        if (PLAYFIELD.y..PLAYFIELD.h).contains(&y) && vp.spans(x, 1) {
            fb.put(x + m, y, c);
        }
    }
    draw_hud_bar(fb, hud.bar());
    mirror_hud_panels(fb);
    for &(e, [r, g, b]) in hud.entries() {
        fb.palette.colors[usize::from(e)] = Rgb6 {
            r: r & 63,
            g: g & 63,
            b: b & 63,
        };
    }
}

/// Fills the side panels of the HUD band (rows 160..191, columns outside m..m + 320) by
/// mirroring the bar's outermost 16 columns outward, repeated as a mirrored repeat (columns
/// 0, 1, .., 15, 15, .., 0, 0, 1, ..), so the band is continuous at every seam. The pipeline
/// darkens the panels by half when it converts the frame to RGBA.
pub fn mirror_hud_panels(fb: &mut Framebuffer) {
    let w = fb.width();
    let m = Viewport::of(fb).margin() as usize;
    if m == 0 {
        return;
    }
    let mirror = |d: usize| {
        let t = d % (2 * MIRRORED);
        if t < MIRRORED {
            t
        } else {
            2 * MIRRORED - 1 - t
        }
    };
    let rows = HUD_Y as usize..HUD_Y as usize + HUD_H;
    for row in fb.pixels[rows.start * w..rows.end * w].chunks_exact_mut(w) {
        for x in 0..m {
            row[x] = row[m + mirror(m - 1 - x)];
        }
        for x in m + HUD_W..w {
            row[x] = row[m + HUD_W - 1 - mirror(x - m - HUD_W)];
        }
    }
}

/// Draws `draw`'s Classic 320 × 200 overlay (text drawn over the finished picture, such as the
/// infra-red notice) into a widescreen frame at x = m. Pixels the overlay does not touch keep
/// the wide picture. On a 320-wide frame `draw` runs directly.
pub fn overlay_centred(fb: &mut Framebuffer, draw: impl Fn(&mut Framebuffer)) {
    let m = Viewport::of(fb).margin() as usize;
    if m == 0 {
        draw(fb);
        return;
    }
    // The overlay on two backgrounds: a pixel it draws has the same index on both.
    let mut a = Framebuffer::new();
    a.palette = fb.palette.clone();
    let mut b = a.clone();
    b.clear(0xFF);
    draw(&mut a);
    draw(&mut b);
    let w = fb.width();
    for y in 0..HEIGHT {
        let row = &mut fb.pixels[y * w + m..y * w + m + WIDTH];
        let (ra, rb) = (
            &a.pixels[y * WIDTH..][..WIDTH],
            &b.pixels[y * WIDTH..][..WIDTH],
        );
        for ((d, &pa), &pb) in row.iter_mut().zip(ra).zip(rb) {
            if pa == pb {
                *d = pa;
            }
        }
    }
    fb.palette = a.palette;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panels_mirror_the_bar_edges() {
        let mut fb = Framebuffer::with_width(428);
        let m = 54;
        for y in 160..192 {
            for x in 0..320 {
                fb.put(m + x, y, x as u8);
            }
        }
        mirror_hud_panels(&mut fb);
        let row = |x: i32| fb.get(x, 170).unwrap();
        // Left: 0, 1, .., 15 outward, then 15, .., 0, then 0, 1, ...
        assert_eq!(row(m - 1), 0);
        assert_eq!(row(m - 16), 15);
        assert_eq!(row(m - 17), 15);
        assert_eq!(row(m - 32), 0);
        assert_eq!(row(m - 33), 0);
        assert_eq!(
            row(0),
            10,
            "distance 53: 21 into the second period, mirrored"
        );
        // Right: 319, 318, .. outward (bar column c holds c mod 256).
        let col = |c: u16| (c % 256) as u8;
        assert_eq!(row(m + 320), col(319));
        assert_eq!(row(m + 335), col(304));
        assert_eq!(row(m + 336), col(304));
        assert_eq!(row(427), col(309));
        // Other rows are untouched.
        assert_eq!(fb.get(0, 159), Some(0));
        assert_eq!(fb.get(0, 192), Some(0));
    }

    #[test]
    fn overlays_land_in_the_centre_and_keep_the_rest() {
        let mut fb = Framebuffer::with_width(384);
        fb.clear(7);
        overlay_centred(&mut fb, |f| {
            f.put(0, 0, 0);
            f.put(10, 5, 0xFF);
            f.put(319, 199, 3);
        });
        assert_eq!(fb.get(32, 0), Some(0));
        assert_eq!(fb.get(42, 5), Some(0xFF));
        assert_eq!(fb.get(351, 199), Some(3));
        assert_eq!(fb.pixels.iter().filter(|&&p| p != 7).count(), 3);
        let mut classic = Framebuffer::new();
        overlay_centred(&mut classic, |f| f.put(1, 1, 9));
        assert_eq!(classic.get(1, 1), Some(9));
    }
}
