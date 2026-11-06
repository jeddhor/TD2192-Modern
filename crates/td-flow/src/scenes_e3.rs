//! Episode 3 scene cards: the 17 scene routines of `td3.exe` and the about/order pictures.
//!
//! Spec: `docs/re/episode-deltas.md` rev 3 §9.2 (E3 table: anchors, styles, inks, waits),
//! `docs/spec/flow/e3-program-flow.md` rev 1 §3.3 (waits; every timed wait flushes the keyboard
//! on entry, §3.5). The cards are drawn by [`crate::scenes_e2::scene_card_with`]; only the
//! layouts are E3's. Fidelity: T0 for the pictures, T2 for the fades and waits.
//!
//! Rows marked "as E1" or "as E2" in the table take that episode's layout:
//! - `tdlab`, `vidcom`: E2's (`crate::scenes_e2`).
//! - `marauder`: "as E1", which E2's `marauder` also is; E2's black-box layout (U-311: the
//!   background 15 rows down, the three captions side by side at (10, 10), (123, 10),
//!   (226, 10), ink 255 = (63, 5, 5)). ASSUMPTION(U-350): no E3 capture shows it.
//! - `vhang`, `intro2`: E1's scene card (text.md §4.5: (10, 170), shadow (−1, +1), entry 210 =
//!   (14, 63, 63), 10 s). ASSUMPTION(U-350): E1's own routines for these two are not
//!   specified apart from the shared row of text.md.
//! - `vlab`: (180, 170) s3 in entry 32 with the picture's colour (episode-deltas §13).
//!
//! `maraud3` ("one joined line at (5, 2)"): black-box C-0041 f946 (captions 7.2, `_`, `_`)
//! confirms the first pen (5, 2), the plain style and the ink. Where the second and third
//! captions go is not shown by any capture. The same phrase describes E2's `marauder`, whose
//! captions are not one concatenated string (drawing them so differs from C-0015 f542 from
//! x = 109 on) but sit at pens 113 and 216 pixels right of the first (U-311).
//! ASSUMPTION(U-350): `maraud3` spaces its captions the same way, at x = 5, 118, 221.
//!
//! `sel-pain` and `melting` draw their third caption at y = 200, off screen (OB-027, kept).

use td_assets::DataDir;
use td_formats::palette::Rgb6;

use crate::scenes_e2::{self, Captions, Layout, Shadow};
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

/// E1's scene card (text.md §4.5): (10, 170), shadow (−1, +1), entry 210 = (14, 63, 63), 10 s.
const fn e1_card(file: &'static str) -> Layout {
    lines(file, (10, 170), Shadow::S1, (210, rgb(14, 63, 63)), 10)
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
                // IM-11 merge: E2 marauder is now a joined line (text.md rev 2 §4.5.1).
                captions: Captions::Joined((5, 2)),
                shadow: Plain,
                ink: (83, rgb(63, 15, 63)),
                secs: 10,
                down: 0,
            },
            Bg::Vid => scenes_e2::Bg::Vid.layout(),
            Bg::Vhg => e1_card("vhang.scr"),
            Bg::In2 => e1_card("intro2.scr"),
            Bg::Sh0 => lines("shuttle0.scr", (10, 170), O4, YELLOW, 15),
            Bg::Vlb => lines("vlab.scr", (180, 170), S3, (32, None), 10),
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
    scenes_e2::scene_card_with(data, assets, &bg.layout(), text).map(|s| s.flushing_waits(true))
}

/// A title card (`Scene_MissionTitle(text, 3Fh, 0, 0)` `04A0:4663`): E1's text card; its wait
/// flushes the keyboard first.
pub fn title_card(assets: &StoryAssets, text: &str) -> StoryScreen {
    crate::story::text_card(assets, text).flushing_waits(true)
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
    fn sel_pain_third_caption_is_off_screen() {
        // OB-027: y0 + 20 = 200.
        let Captions::Lines((_, y)) = Bg::Sel.layout().captions else {
            panic!("lines");
        };
        assert_eq!(y + 20, 200);
    }
}
