//! Episode 3 scene cards: the 17 scene routines of `td3.exe` and the about/order pictures.
//!
//! Spec: `docs/re/episode-deltas.md` rev 3 §9.2 (E3 table: anchors, styles, inks, waits),
//! `docs/spec/flow/e3-program-flow.md` rev 1 §3.3 (waits; every timed wait flushes the keyboard
//! on entry, §3.5). The cards are drawn by [`crate::scene_card`]; only the
//! layouts are E3's. Fidelity: T0 for the pictures, T2 for the fades and waits.
//!
//! Rows marked "as E1" or "as E2" in the table take that episode's layout (E1: render/text
//! rev 2 §4.5.1):
//! - `tdlab`, `vidcom`: E2's (`crate::scenes_e2`).
//! - `marauder`: E1's, which E2's `marauder` also uses, so it is taken from `crate::scenes_e2`
//!   (the joined caption line of text §4.5.1, the background 15 rows down).
//! - `vhang`, `intro2`, `vlab`: E1's routines (`crate::story::e1_scene_layout`): (20, 170) s3,
//!   entry 165 = (14, 38, 63); (50, 90) plain, entry 140 = (0, 0, 0); (180, 170) s3, entry 32 =
//!   (14, 38, 63); 10 s each.
//!
//! `maraud3` ("one joined line at (5, 2)", plain): the captions joined as `marauder` joins them
//! (A + 4 spaces + B + 3 spaces + C + 3 spaces, text §4.5.1, `Captions::Joined`) at (5, 2).
//! Black-box C-0041 f946 (captions 7.2, `_`, `_`) confirms the pen, the style and the ink;
//! ASSUMPTION(U-350): the separators are `marauder`'s (no capture with a second caption).
//!
//! `sel-pain` and `melting` draw their third caption at y = 200, off screen (OB-027, kept).

use td_assets::DataDir;
use td_formats::palette::{Palette, Rgb6};

use crate::scene_card::{self as card, Captions, Layout, Shadow};
use crate::scenes_e2;
use crate::story::{StoryAssets, StoryError, StoryScreen};

/// The backgrounds of the E3 scene routines (e3-program-flow §4.4 abbreviations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bg {
    /// `command` (`cmd`).
    Cmd,
    /// `convers` (`cnv`).
    Cnv,
    /// `intro2` (`in2`).
    In2,
    /// `ionstat` (`ion`).
    Ion,
    /// `marauder` (`mar`).
    Mar,
    /// `melting` (`mel`), 15 s.
    Mel,
    /// `mining1` (`mn1`).
    Mn1,
    /// `mining2` (`mn2`).
    Mn2,
    /// `maraud2` (`mr2`).
    Mr2,
    /// `maraud3` (`mr3`).
    Mr3,
    /// `kolorvci` (`kol`).
    Kol,
    /// `sel-pain` (`sel`), 15 s.
    Sel,
    /// `shuttle0` (`sh0`), 15 s.
    Sh0,
    /// `vhang` (`vhg`).
    Vhg,
    /// `vidcom` (`vid`).
    Vid,
    /// `vlab` (`vlb`).
    Vlb,
    /// `tdlab` (`lab`).
    Lab,
    /// `aboutus` (menu item 6), 30 s.
    Aboutus,
    /// `order` (menu item 7), 45 s.
    Order,
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

const fn picture(file: &'static str, secs: u32) -> Layout {
    Layout {
        file,
        captions: Captions::None,
        shadow: Shadow::Plain,
        ink: (0, None),
        secs,
        down: 0,
    }
}

/// E1's layout of `file` (render/text rev 2 §4.5.1).
fn e1(file: &str) -> Layout {
    *crate::story::e1_scene_layout(file).expect("an E1 scene card")
}

impl Bg {
    /// The card's layout (episode-deltas rev 3 §9.2, `td3:04A0:1D6B..449C`).
    pub fn layout(self) -> Layout {
        use Shadow::*;
        const YELLOW: (u8, Option<Rgb6>) = (255, rgb(63, 63, 14));
        const RED: (u8, Option<Rgb6>) = (255, rgb(63, 14, 14));
        match self {
            Bg::Aboutus => picture("aboutus.scr", 30),
            Bg::Order => picture("order.scr", 45),
            Bg::Lab => scenes_e2::Bg::Lab.layout(),
            Bg::Mar => scenes_e2::Bg::Mar.layout(),
            Bg::Mr2 => lines("maraud2.scr", (180, 170), O6, (255, rgb(63, 5, 5)), 10),
            Bg::Mr3 => Layout {
                file: "maraud3.scr",
                captions: Captions::Joined((5, 2)),
                shadow: Plain,
                ink: (83, rgb(63, 15, 63)),
                secs: 10,
                down: 0,
            },
            Bg::Vid => scenes_e2::Bg::Vid.layout(),
            Bg::Vhg => e1("vhang.scr"),
            Bg::In2 => e1("intro2.scr"),
            Bg::Sh0 => lines("shuttle0.scr", (10, 170), O4, YELLOW, 15),
            Bg::Vlb => e1("vlab.scr"),
            Bg::Cnv => lines("convers.scr", (10, 10), Plain, RED, 10),
            Bg::Sel => lines("sel-pain.scr", (120, 180), Plain, RED, 15),
            Bg::Mel => lines("melting.scr", (120, 180), Plain, RED, 15),
            Bg::Kol => lines("kolorvci.scr", (180, 170), O4, YELLOW, 10),
            Bg::Cmd => lines("command.scr", (10, 5), Plain, (255, rgb(63, 63, 63)), 10),
            Bg::Mn1 => lines("mining1.scr", (10, 170), O4, YELLOW, 10),
            Bg::Ion => lines("ionstat.scr", (10, 170), O4, YELLOW, 10),
            Bg::Mn2 => lines("mining2.scr", (10, 170), Plain, RED, 10),
        }
    }
}

/// An E3 scene card: the background, its captions, then a timed wait that flushes the
/// keyboard first (or a key; Esc sets the skip flag), with the usual fades.
pub fn scene_card(
    data: &DataDir,
    assets: &StoryAssets,
    bg: Bg,
    text: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    card::scene_card(data, assets, &bg.layout(), text).map(|s| s.flushing_waits(true))
}

/// A title card (`Scene_MissionTitle(text, 3Fh, 0, 0)` `04A0:4663`): E1's text card; its wait
/// flushes the keyboard first.
pub fn title_card(assets: &StoryAssets, text: &str, left: &Palette) -> StoryScreen {
    crate::story::text_card(assets, text, left).flushing_waits(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits() {
        // e3-program-flow §3.3: 15 s for shuttle0, sel-pain, melting; order 45 s.
        for bg in [Bg::Sh0, Bg::Sel, Bg::Mel] {
            assert_eq!(bg.layout().secs, 15, "{bg:?}");
        }
        assert_eq!(Bg::Order.layout().secs, 45);
        assert_eq!(Bg::Aboutus.layout().secs, 30);
        assert_eq!(Bg::Kol.layout().secs, 10);
        assert_eq!(Bg::Sel.layout().file, "sel-pain.scr");
    }

    #[test]
    fn as_e1_cards() {
        // render/text rev 2 §4.5.1.
        let l = Bg::Vhg.layout();
        assert_eq!(
            (l.captions, l.shadow, l.ink.0),
            (Captions::Lines((20, 170)), Shadow::S3, 165)
        );
        assert_eq!(Bg::In2.layout().captions, Captions::Lines((50, 90)));
        assert_eq!(Bg::Vlb.layout().ink, (32, card::rgb(14, 38, 63)));
        assert_eq!(Bg::Mar.layout().down, 15);
    }

    #[test]
    fn sel_pain_third_caption_is_off_screen() {
        // OB-027: y0 + 20 = 200.
        let Captions::Lines((_, y)) = Bg::Sel.layout().captions else {
            panic!("lines");
        };
        assert_eq!(y + 20, 200);
    }
}
