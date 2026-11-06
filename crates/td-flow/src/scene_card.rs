//! Scene cards of every episode: a `.scr` background, three captions and a timed wait. The
//! routines differ per background (caption anchor, black copies, ink, wait); this module draws
//! a card from its [`Layout`]. The per-episode tables are `crate::story::e1_scene_layout`
//! (E1) and `crate::scenes_e2::Bg::layout` (E2).
//!
//! Spec: `docs/spec/render/text.md` rev 2 §4.5.1 (E1 cards, shadow styles, the `marauder`
//! joined line and the `voffice` centring), `docs/re/episode-deltas.md` §9.2 (E2/E3 cards);
//! OB-003 (backgrounds plotted one pixel to the right). Fidelity: T0 for the picture, T2 for
//! the fades and waits (`crate::story`).

use td_assets::DataDir;
use td_formats::palette::Rgb6;
use td_formats::screen::Screen as ScrImage;
use td_render::small_text;
use td_render::{Framebuffer, WIDTH};

use crate::story::{Hold, StoryAssets, StoryError, StoryScreen};

/// Black copies drawn before the ink (text.md §4.5.1 shadow styles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shadow {
    /// `s1`: (−1, +1).
    S1,
    /// `s3`: (−1, +1), (−1, 0), (0, +1).
    S3,
    /// `o6`: `s3` plus (+1, −1), (+1, 0), (0, −1).
    O6,
    /// `o4`: (+1, 0), (−1, 0), (0, −1), (0, +1).
    O4,
    /// `l`: (−1, 0) (the `marauder` joined line).
    L,
    /// `p`: none.
    Plain,
}

impl Shadow {
    /// Pen offsets of the black copies, in drawing order.
    pub fn offsets(self) -> &'static [(i32, i32)] {
        match self {
            Shadow::S1 => &[(-1, 1)],
            Shadow::S3 => &[(-1, 1), (-1, 0), (0, 1)],
            Shadow::O6 => &[(-1, 1), (-1, 0), (0, 1), (1, -1), (1, 0), (0, -1)],
            Shadow::O4 => &[(1, 0), (-1, 0), (0, -1), (0, 1)],
            Shadow::L => &[(-1, 0)],
            Shadow::Plain => &[],
        }
    }
}

/// How a card places its captions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captions {
    /// Three lines at the anchor, y + 0, + 10, + 20.
    Lines((i32, i32)),
    /// The captions joined into one string, A + 4 spaces + B + 3 spaces + C + 3 spaces, at the
    /// pen (`marauder`, text.md §4.5.1): B and C land where the caption lengths put them.
    Joined((i32, i32)),
    /// Each line centred on its own length, x = 160 − Round(5 × length / 2), at the given pen
    /// y (E1 `voffice`, E2 `hyde`, `velvswlf`).
    Centred([i32; 3]),
    /// A picture without text (`aboutus`, `order`).
    None,
}

/// One card's layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub file: &'static str,
    pub captions: Captions,
    pub shadow: Shadow,
    /// The ink entry and the colour the routine writes into it (`None`: not set).
    pub ink: (u8, Option<Rgb6>),
    /// Seconds before the card ends without a key.
    pub secs: u32,
    /// Rows the background is plotted down (15 for `marauder`, text.md §4.5.1).
    pub down: i32,
}

/// `Some(Rgb6)` in a `const` table.
pub const fn rgb(r: u8, g: u8, b: u8) -> Option<Rgb6> {
    Some(Rgb6 { r, g, b })
}

/// A card with three caption lines at `at`, the background not moved down.
pub const fn lines(
    file: &'static str,
    at: (i32, i32),
    shadow: Shadow,
    ink: (u8, Option<Rgb6>),
    secs: u32,
) -> Layout {
    Layout {
        file,
        captions: Captions::Lines(at),
        shadow,
        ink,
        secs,
        down: 0,
    }
}

/// `Round(n / 2)` for n ≥ 0 (Real48 `Round`: halves away from zero).
fn round_half(n: i32) -> i32 {
    (n + 1).div_euclid(2)
}

/// The pen x of a centred caption: 160 − Round(5 × length / 2).
pub fn centred_x(text: &str) -> i32 {
    160 - round_half(5 * text.chars().count() as i32)
}

/// The `marauder` caption string: A + 4 spaces + B + 3 spaces + C + 3 spaces.
pub fn joined(text: [&str; 3]) -> String {
    format!("{}    {}   {}   ", text[0], text[1], text[2])
}

/// Draws `text` with the shadow copies in colour 0, then in the ink (transparent small font).
fn caption(fb: &mut Framebuffer, a: &StoryAssets, text: &str, pen: (i32, i32), l: &Layout) {
    for &(dx, dy) in l.shadow.offsets() {
        small_text::draw(fb, &a.small, text, (pen.0 + dx, pen.1 + dy), 0);
    }
    small_text::draw(fb, &a.small, text, pen, l.ink.0);
}

/// A scene card from its layout: the background at x + 1 (and `down` rows lower; rows past
/// the screen are clipped), the captions, the ink entry; then a timed wait (or a key; Esc sets
/// the skip flag), with the usual fades.
pub fn scene_card(
    data: &DataDir,
    assets: &StoryAssets,
    l: &Layout,
    text: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    let scr = ScrImage::parse(&data.read(l.file)?)?;
    let mut s = StoryScreen::blank(Hold::Seconds(l.secs));
    let (fb, pal) = s.parts_mut();
    *pal = scr.palette.clone();
    for (row, line) in scr.pixels.chunks_exact(WIDTH).enumerate() {
        for (col, &p) in line.iter().enumerate() {
            fb.put(col as i32 + 1, row as i32 + l.down, p);
        }
    }
    if let (e, Some(c)) = l.ink {
        pal.colors[usize::from(e)] = c;
    }
    match l.captions {
        Captions::Lines((x, y)) => {
            for (k, t) in text.iter().enumerate() {
                caption(fb, assets, t, (x, y + 10 * k as i32), l);
            }
        }
        Captions::Joined(pen) => caption(fb, assets, &joined(text), pen, l),
        Captions::Centred(ys) => {
            for (t, y) in text.iter().zip(ys) {
                caption(fb, assets, t, (centred_x(t), y), l);
            }
        }
        Captions::None => {}
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_styles() {
        assert_eq!(Shadow::S1.offsets().len(), 1);
        assert_eq!(Shadow::S3.offsets().len(), 3);
        assert_eq!(Shadow::O6.offsets().len(), 6);
        assert_eq!(&Shadow::O6.offsets()[..3], Shadow::S3.offsets());
        assert_eq!(Shadow::O4.offsets().len(), 4);
        assert_eq!(Shadow::L.offsets(), &[(-1, 0)]);
        assert!(Shadow::Plain.offsets().is_empty());
    }

    #[test]
    fn centring_and_joining() {
        // text.md §4.5.1: a 23-character line gets x = 160 − 58 = 102.
        assert_eq!(centred_x(&"a".repeat(23)), 102);
        assert_eq!(centred_x("ab"), 155);
        assert_eq!(joined(["a", "b", "c"]), "a    b   c   ");
    }
}
