//! Colour-blind guidance line (M12-04; accessibility setting `colorblind_guidance`, spec
//! `docs/spec/frontend/port-front-end.md` rev 2 §3.3). Fidelity: T3, off by default.
//!
//! The original draws the six guidance dots in palette entries the playfield shares: a dark to
//! bright red ramp for a target, a green ramp for a friend and a cyan ramp for home
//! (`hud.md` §3.4). Red against green is the pair the commonest colour-vision deficiencies
//! (protanopia, deuteranopia) confuse, and the dark end of each ramp disappears on dark
//! ground. With the setting on, the line is drawn at the RGBA stage instead, so no palette
//! entry changes:
//! - each dot in one flat core colour per marker, taken from the Okabe–Ito set (designed to
//!   stay apart under protanopia, deuteranopia and tritanopia) and also differing in lightness:
//!   target orange (230, 159, 0), friend sky blue (86, 180, 233), home white (255, 255, 255);
//! - every neighbour of a dot (8-neighbourhood) that is not itself a dot in black, so each
//!   colour is paired with a dark outline and stands out on light and dark ground alike.
//!
//! The marks are drawn after the palette stage (so the upscaler sees them) and again over the
//! final image (so lighting and bloom leave them at full strength). With the setting off there
//! are no marks and the output is the Classic one.

use crate::flight::PLAYFIELD;
use crate::hud::{Hud, MARKER_FRIEND, MARKER_HOME, MARKER_TARGET};

/// A pixel of the base frame (W × 200) drawn in a fixed colour by the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub x: i32,
    pub y: i32,
    pub rgb: [u8; 3],
}

/// Core colour of a target's line (Okabe–Ito orange).
pub const TARGET: [u8; 3] = [230, 159, 0];
/// Core colour of a friend's line (Okabe–Ito sky blue).
pub const FRIEND: [u8; 3] = [86, 180, 233];
/// Core colour of the home line.
pub const HOME: [u8; 3] = [255, 255, 255];
/// The outline.
pub const OUTLINE: [u8; 3] = [0, 0, 0];

/// The core colour for a guidance marker (`hud::MARKER_*`).
pub fn colour(marker: u8) -> Option<[u8; 3]> {
    match marker {
        MARKER_TARGET => Some(TARGET),
        MARKER_FRIEND => Some(FRIEND),
        MARKER_HOME => Some(HOME),
        _ => None,
    }
}

/// The marks of the colour-blind guidance line for the instruments' last step, in a frame
/// whose Classic picture starts at column `margin`: the outline first, then the dots. Pixels
/// outside the playfield rows (0..160) or the frame's `width` are left out, as the dots are.
pub fn marks(hud: &Hud, margin: i32, width: i32) -> Vec<Mark> {
    marks_of(hud.dots(), hud.guidance_marker(), margin, width)
}

/// [`marks`] from the instruments' guidance pixels and marker.
fn marks_of(pixels: &[(i32, i32, u8)], marker: Option<u8>, margin: i32, width: i32) -> Vec<Mark> {
    let Some(core) = marker.and_then(colour) else {
        return Vec::new();
    };
    // The six dots come after their 18 shadow pixels (`Hud::dots`).
    let dots: Vec<(i32, i32)> = pixels
        .iter()
        .skip(18)
        .map(|&(x, y, _)| (x + margin, y))
        .collect();
    let inside =
        |x: i32, y: i32| (0..width).contains(&x) && (PLAYFIELD.y..PLAYFIELD.h).contains(&y);
    let mut out: Vec<Mark> = Vec::new();
    for &(x, y) in &dots {
        for (dx, dy) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let (nx, ny) = (x + dx, y + dy);
            if inside(nx, ny)
                && !dots.contains(&(nx, ny))
                && !out.iter().any(|m| (m.x, m.y) == (nx, ny))
            {
                out.push(Mark {
                    x: nx,
                    y: ny,
                    rgb: OUTLINE,
                });
            }
        }
    }
    out.extend(
        dots.iter()
            .filter(|&&(x, y)| inside(x, y))
            .map(|&(x, y)| Mark { x, y, rgb: core }),
    );
    out
}

/// Draws `marks` into an RGBA8 image of `w` pixels per row in which each base pixel is a
/// `k` × `k` block.
pub fn draw(out: &mut [[u8; 4]], w: usize, k: usize, marks: &[Mark]) {
    let row = w * k;
    for m in marks {
        let (Ok(x), Ok(y)) = (usize::try_from(m.x), usize::try_from(m.y)) else {
            continue;
        };
        if x >= w {
            continue;
        }
        let [r, g, b] = m.rgb;
        for sy in y * k..(y + 1) * k {
            if let Some(px) = out.get_mut(sy * row + x * k..sy * row + (x + 1) * k) {
                px.fill([r, g, b, 255]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Linearised sRGB channel.
    fn lin(c: u8) -> f64 {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Simulates a dichromat in linear RGB: the Viénot, Brettel and Mollon (1999) projections
    /// for protanopia and deuteranopia, and a single-plane approximation of Brettel, Viénot
    /// and Mollon (1997) for tritanopia (kind 0, 1, 2).
    fn simulate(rgb: [u8; 3], kind: usize) -> [f64; 3] {
        let [r, g, b] = rgb.map(lin);
        let m: [[f64; 3]; 3] = match kind {
            0 => [
                [0.11238, 0.88762, 0.0],
                [0.11238, 0.88762, 0.0],
                [0.00401, -0.00401, 1.0],
            ],
            1 => [
                [0.29275, 0.70725, 0.0],
                [0.29275, 0.70725, 0.0],
                [-0.02234, 0.02234, 1.0],
            ],
            _ => [
                [1.0, 0.14461, -0.14461],
                [0.0, 0.85924, 0.14076],
                [0.0, 0.85924, 0.14076],
            ],
        };
        [0, 1, 2].map(|i| m[i][0] * r + m[i][1] * g + m[i][2] * b)
    }

    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        a.iter()
            .zip(&b)
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    #[test]
    fn the_colours_stay_apart_for_every_common_deficiency() {
        let colours = [TARGET, FRIEND, HOME];
        for kind in 0..3 {
            for i in 0..3 {
                for j in i + 1..3 {
                    let d = distance(simulate(colours[i], kind), simulate(colours[j], kind));
                    assert!(
                        d > 0.25,
                        "kind {kind}: colours {i} and {j} too close ({d:.3})"
                    );
                }
                let d = distance(simulate(colours[i], kind), simulate(OUTLINE, kind));
                assert!(
                    d > 0.3,
                    "kind {kind}: colour {i} too close to the outline ({d:.3})"
                );
            }
        }
    }

    /// The C-0002 f3543 line (hud.md §6 vector 4): dots from (169, 67) up to (172, 63).
    fn line() -> Vec<(i32, i32, u8)> {
        let pos = [
            (169, 67),
            (170, 66),
            (170, 66),
            (171, 65),
            (171, 64),
            (172, 63),
        ];
        let mut px: Vec<(i32, i32, u8)> = pos
            .iter()
            .flat_map(|&(x, y)| [(x + 1, y, 0), (x + 1, y + 1, 0), (x, y + 1, 0)])
            .collect();
        px.extend(pos.iter().map(|&(x, y)| (x, y, 0xB0)));
        px
    }

    #[test]
    fn marks_colour_the_dots_by_marker_and_outline_them() {
        let m = marks_of(&line(), Some(MARKER_TARGET), 0, 320);
        let dots: Vec<_> = m.iter().filter(|m| m.rgb == TARGET).collect();
        assert_eq!(
            dots.len(),
            6,
            "one mark per dot (a repeated position twice)"
        );
        let outline: Vec<_> = m.iter().filter(|m| m.rgb == OUTLINE).collect();
        // Every outline pixel touches a dot and is not one; no pixel twice.
        let at = |x, y| dots.iter().any(|d| (d.x, d.y) == (x, y));
        for o in &outline {
            assert!(!at(o.x, o.y));
            assert!((-1..=1).any(|dx| (-1..=1).any(|dy| at(o.x + dx, o.y + dy))));
        }
        let mut seen: Vec<_> = outline.iter().map(|o| (o.x, o.y)).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), outline.len());
        // The outline comes first, so the dots win.
        assert!(m.iter().position(|m| m.rgb == TARGET).unwrap() >= outline.len());
        // Friend and home, a widescreen margin, and no line.
        let f = marks_of(&line(), Some(MARKER_FRIEND), 54, 428);
        assert!(
            f.iter()
                .any(|m| m.rgb == FRIEND && (m.x, m.y) == (169 + 54, 67))
        );
        assert!(
            marks_of(&line(), Some(MARKER_HOME), 0, 320)
                .iter()
                .any(|m| m.rgb == HOME)
        );
        assert!(marks_of(&line(), None, 0, 320).is_empty());
        assert!(marks_of(&line(), Some(0x77), 0, 320).is_empty());
        // Pixels outside the frame or the playfield are left out.
        let edge: Vec<(i32, i32, u8)> = (0..24)
            .map(|i| (319, 159, if i < 18 { 0 } else { 1 }))
            .collect();
        let e = marks_of(&edge, Some(MARKER_TARGET), 0, 320);
        assert!(e.iter().all(|m| m.x < 320 && m.y < 160), "{e:?}");
    }

    #[test]
    fn draw_fills_blocks_and_skips_what_is_outside() {
        let (w, k) = (4, 2);
        let mut out = vec![[9, 9, 9, 255]; w * k * 3 * k];
        draw(
            &mut out,
            w,
            k,
            &[
                Mark {
                    x: 1,
                    y: 1,
                    rgb: [1, 2, 3],
                },
                Mark {
                    x: 4,
                    y: 0,
                    rgb: [7, 7, 7],
                },
                Mark {
                    x: -1,
                    y: 0,
                    rgb: [7, 7, 7],
                },
            ],
        );
        let row = w * k;
        let set: Vec<usize> = (0..out.len())
            .filter(|&i| out[i] != [9, 9, 9, 255])
            .collect();
        assert_eq!(set, [2 * row + 2, 2 * row + 3, 3 * row + 2, 3 * row + 3]);
        assert_eq!(out[2 * row + 2], [1, 2, 3, 255]);
    }
}
