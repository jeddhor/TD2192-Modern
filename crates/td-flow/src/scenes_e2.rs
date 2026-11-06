//! Episode 2 scene cards: one routine per background, each with its own caption anchor,
//! shadow, ink and wait.
//!
//! Spec: `docs/re/episode-deltas.md` rev 2 §9.2 (anchors, styles, inks, waits),
//! `docs/spec/flow/e2-program-flow.md` rev 1 §3.3 (waits, about/order), `docs/spec/render/text.md`
//! rev 1 §4.5 (three caption lines 10 pixels apart). Fidelity: T0 for the pictures, T2 for the
//! fades and waits (as the E1 cards, `crate::story`).
//!
//! Black-box observations (C-0015 f542, C-0016 f1318), recorded as U-311:
//! - every background is plotted one pixel to the right (x = 1..320, as E1's OB-003);
//! - the `marauder` card ("one joined line, as E1") plots its background 15 rows down (screen
//!   rows 15..199 show file rows 0..184) and draws the three captions on one row, pens
//!   (10, 10), (123, 10), (226, 10), no shadow. ASSUMPTION(U-311): those pens are fixed; with
//!   one capture (captions p.0/p.1/p.2, 23/22/11 characters) they could also derive from the
//!   caption lengths.
//!
//! ASSUMPTION(U-311): `hyde` and `velvswlf` ("centred x, y 5/15/25, outline (E1 voffice
//! style)") centre each line as the E1 still captions do (x = 160 − Round(5 × length / 2)) and
//! outline it with the four-neighbour style `o4`; no capture shows them yet.

use td_assets::DataDir;
use td_formats::palette::Rgb6;
use td_formats::screen::Screen as ScrImage;
use td_render::small_text;
use td_render::{Framebuffer, WIDTH};

use crate::story::{Hold, StoryAssets, StoryError, StoryScreen};

/// Black copies drawn before the ink (episode-deltas §9.2 shadow styles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shadow {
    /// `s1`: (−1, +1).
    S1,
    /// `s3`: (−1, +1), (−1, 0), (0, +1).
    S3,
    /// `o6`: `s3` plus (+1, −1), (+1, 0), (0, −1).
    O6,
    /// `o4`: (±1, 0), (0, ±1).
    O4,
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
            Shadow::O4 => &[(1, 0), (-1, 0), (0, 1), (0, -1)],
            Shadow::Plain => &[],
        }
    }
}

/// How a card places its captions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captions {
    /// Three lines at the anchor, y + 0, + 10, + 20.
    Lines((i32, i32)),
    /// The three captions side by side on one row: pen x of each, pen y (`marauder`).
    Row([i32; 3], i32),
    /// Each line centred on x = 160 at the given pen y (`hyde`, `velvswlf`).
    Centred([i32; 3]),
    /// A picture without text (`aboutus`, `order`).
    None,
}

/// The backgrounds of the E2 scene routines (e2-program-flow §4.4 abbreviations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bg {
    /// `sbar` (`bar`).
    Bar,
    Bb,
    /// `briefing` (`brf`).
    Brf,
    /// `bunker` (`bun`).
    Bun,
    /// `dieseche` (`die`).
    Die,
    /// `elevator` (`elv`).
    Elv,
    /// `greenblo` (`grn`).
    Grn,
    /// `hydedead` (`hdd`).
    Hdd,
    /// `helihangar` → `helihang.scr` (`heli`).
    Heli,
    /// `hangar` (`hg`).
    Hg,
    /// `hangar2` (`hg2`).
    Hg2,
    /// `hyde` (`hyd`).
    Hyd,
    /// `intro2` (`in2`).
    In2,
    /// `i-port` (`ipt`).
    Ipt,
    /// `v-port` (`vpt`).
    Vpt,
    /// `tdlab` (`lab`).
    Lab,
    /// `marauder` (`mar`).
    Mar,
    /// `sewer` (`sew`).
    Sew,
    /// `shooting` (`sho`).
    Sho,
    /// `shuttle` (`shu`).
    Shu,
    /// `soffice` (`sof`).
    Sof,
    /// `tdoffic2` (`to2`).
    To2,
    /// `velsroom` (`vel`).
    Vel,
    /// `vidcom` (`vid`).
    Vid,
    /// `velvswlf` (`vvw`).
    Vvw,
    /// `aboutus` (menu item 6).
    Aboutus,
    /// `order` (menu item 7).
    Order,
}

/// One card's layout (episode-deltas §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub file: &'static str,
    pub captions: Captions,
    pub shadow: Shadow,
    /// The ink entry and the colour the routine writes into it (`None`: not set).
    pub ink: (u8, Option<Rgb6>),
    /// Seconds before the card ends without a key.
    pub secs: u32,
    /// Rows the background is plotted down (U-311: 15 for `marauder`).
    pub down: i32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Option<Rgb6> {
    Some(Rgb6 { r, g, b })
}

const fn lines(
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

impl Bg {
    /// The card's layout (episode-deltas rev 2 §9.2, `td2:04EC:1D4A..6127`).
    pub fn layout(self) -> Layout {
        use Shadow::*;
        match self {
            Bg::Hg => lines("hangar.scr", (10, 170), S1, (210, rgb(14, 63, 63)), 10),
            Bg::Hg2 => lines("hangar2.scr", (10, 170), S1, (97, rgb(14, 38, 63)), 10),
            Bg::Lab => lines("tdlab.scr", (10, 170), S3, (109, rgb(10, 10, 63)), 10),
            Bg::Mar => Layout {
                file: "marauder.scr",
                captions: Captions::Row([10, 123, 226], 10),
                shadow: Plain,
                ink: (255, rgb(63, 5, 5)),
                secs: 10,
                down: 15,
            },
            Bg::Brf => lines("briefing.scr", (10, 170), S3, (250, rgb(10, 10, 63)), 10),
            Bg::Heli => lines("helihang.scr", (180, 20), Plain, (255, rgb(10, 0, 0)), 10),
            Bg::In2 => lines("intro2.scr", (50, 90), Plain, (140, rgb(0, 0, 0)), 10),
            Bg::Sho => lines("shooting.scr", (10, 170), Plain, (28, rgb(14, 14, 63)), 10),
            Bg::Die => lines("dieseche.scr", (10, 170), O6, (174, rgb(63, 0, 63)), 15),
            Bg::Sof => lines("soffice.scr", (180, 170), O6, (74, rgb(14, 50, 63)), 10),
            Bg::Sew => lines("sewer.scr", (180, 170), O6, (194, rgb(14, 50, 63)), 10),
            Bg::Shu => lines("shuttle.scr", (10, 10), O6, (15, rgb(14, 63, 14)), 10),
            Bg::Bun => lines("bunker.scr", (10, 10), O6, (197, rgb(40, 40, 63)), 10),
            Bg::Elv => lines("elevator.scr", (10, 10), O6, (255, rgb(63, 40, 40)), 10),
            Bg::Grn => lines("greenblo.scr", (120, 170), O6, (255, rgb(40, 63, 40)), 15),
            Bg::Vid => lines("vidcom.scr", (10, 10), O6, (255, rgb(40, 40, 63)), 10),
            Bg::Bb => lines("bb.scr", (10, 10), O6, (255, rgb(63, 40, 40)), 10),
            Bg::Vpt => lines("v-port.scr", (240, 10), O6, (255, rgb(40, 63, 40)), 10),
            Bg::Ipt => lines("i-port.scr", (240, 10), O6, (255, rgb(40, 63, 40)), 10),
            Bg::Hdd => lines("hydedead.scr", (240, 10), O6, (255, rgb(40, 63, 40)), 15),
            Bg::To2 => lines("tdoffic2.scr", (10, 170), O6, (4, rgb(14, 50, 63)), 10),
            Bg::Bar => lines("sbar.scr", (10, 10), S1, (90, rgb(48, 38, 63)), 10),
            Bg::Vel => lines("velsroom.scr", (10, 20), S3, (12, rgb(40, 40, 63)), 10),
            Bg::Hyd => Layout {
                file: "hyde.scr",
                captions: Captions::Centred([5, 15, 25]),
                shadow: O4,
                ink: (3, rgb(63, 63, 63)),
                secs: 10,
                down: 0,
            },
            Bg::Vvw => Layout {
                file: "velvswlf.scr",
                captions: Captions::Centred([5, 15, 25]),
                shadow: O4,
                ink: (3, None),
                secs: 15,
                down: 0,
            },
            Bg::Aboutus => Layout {
                file: "aboutus.scr",
                captions: Captions::None,
                shadow: Plain,
                ink: (0, None),
                secs: 30,
                down: 0,
            },
            Bg::Order => Layout {
                file: "order.scr",
                captions: Captions::None,
                shadow: Plain,
                ink: (0, None),
                secs: 30,
                down: 0,
            },
        }
    }
}

/// `Round(n / 2)` for n ≥ 0 (halves away from zero), as the E1 still captions.
fn round_half(n: i32) -> i32 {
    (n + 1).div_euclid(2)
}

/// Draws `text` with the shadow copies in colour 0, then in `ink`.
fn caption(fb: &mut Framebuffer, a: &StoryAssets, text: &str, pen: (i32, i32), l: &Layout) {
    for &(dx, dy) in l.shadow.offsets() {
        small_text::draw(fb, &a.small, text, (pen.0 + dx, pen.1 + dy), 0);
    }
    small_text::draw(fb, &a.small, text, pen, l.ink.0);
}

/// A scene card: the background, its captions, then a timed wait (or a key; Esc sets the skip
/// flag), with the usual fades.
pub fn scene_card(
    data: &DataDir,
    assets: &StoryAssets,
    bg: Bg,
    text: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    scene_card_with(data, assets, &bg.layout(), text)
}

/// A scene card with layout `l` (shared with the E3 cards, `crate::scenes_e3`).
pub fn scene_card_with(
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
        Captions::Row(xs, y) => {
            for (t, x) in text.iter().zip(xs) {
                caption(fb, assets, t, (x, y), l);
            }
        }
        Captions::Centred(ys) => {
            for (t, y) in text.iter().zip(ys) {
                let x = 160 - round_half(5 * t.chars().count() as i32);
                caption(fb, assets, t, (x, y), l);
            }
        }
        Captions::None => {}
    }
    Ok(s)
}

/// A title card (`Scene_MissionTitle(text, 3Fh, 0, 0)` `04EC:5A89`): E1's text card.
pub fn title_card(assets: &StoryAssets, text: &str) -> StoryScreen {
    crate::story::text_card(assets, text)
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
        assert!(Shadow::Plain.offsets().is_empty());
    }

    #[test]
    fn long_cards_wait_15_seconds() {
        // e2-program-flow §3.3: dieseche, greenblo, hydedead, velvswlf.
        for bg in [Bg::Die, Bg::Grn, Bg::Hdd, Bg::Vvw] {
            assert_eq!(bg.layout().secs, 15, "{bg:?}");
        }
        assert_eq!(Bg::To2.layout().secs, 10);
        assert_eq!(Bg::Aboutus.layout().secs, 30);
        assert_eq!(Bg::Heli.layout().file, "helihang.scr", "8.3 name");
    }
}
