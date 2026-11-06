//! Episode 2 scene cards: one routine per background, each with its own caption anchor,
//! shadow, ink and wait.
//!
//! Spec: `docs/re/episode-deltas.md` §9.2 (anchors, styles, inks, waits),
//! `docs/spec/render/text.md` rev 2 §4.5.1 (the `marauder` joined line and background offset;
//! the `voffice` centring and `o4` outline that `hyde` and `velvswlf` use 5 rows higher; U-311
//! resolved), `docs/spec/flow/e2-program-flow.md` rev 1 §3.3 (waits, about/order). Fidelity:
//! T0 for the pictures, T2 for the fades and waits (as the E1 cards, `crate::story`). Drawn by
//! [`crate::scene_card`]. Matched by C-0015 f542 (`marauder`), C-0016 f1318 (`tdoffic2`) and
//! C-0046 (`hyde`).

use td_assets::DataDir;

pub use crate::scene_card::{Captions, Layout, Shadow};
use crate::scene_card::{lines, rgb};
use crate::story::{StoryAssets, StoryError, StoryScreen};

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

impl Bg {
    /// The card's layout (episode-deltas §9.2, `td2:04EC:1D4A..6127`).
    pub fn layout(self) -> Layout {
        use Shadow::*;
        match self {
            Bg::Hg => lines("hangar.scr", (10, 170), S1, (210, rgb(14, 63, 63)), 10),
            Bg::Hg2 => lines("hangar2.scr", (10, 170), S1, (97, rgb(14, 38, 63)), 10),
            Bg::Lab => lines("tdlab.scr", (10, 170), S3, (109, rgb(10, 10, 63)), 10),
            // As E1 (text.md §4.5.1): 15 rows down, one joined line at (10, 10), style l.
            Bg::Mar => Layout {
                file: "marauder.scr",
                captions: Captions::Joined((10, 10)),
                shadow: L,
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
            // E1 voffice's layout 5 rows higher (text.md §4.5.1; C-0046).
            Bg::Hyd => Layout {
                file: "hyde.scr",
                captions: Captions::Centred([5, 15, 25]),
                shadow: O4,
                ink: (3, rgb(63, 63, 63)),
                secs: 10,
                down: 0,
            },
            // As hyde; entry 3 keeps the picture's own colour.
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

/// A scene card: the background, its captions, then a timed wait (or a key; Esc sets the skip
/// flag), with the usual fades.
pub fn scene_card(
    data: &DataDir,
    assets: &StoryAssets,
    bg: Bg,
    text: [&str; 3],
) -> Result<StoryScreen, StoryError> {
    crate::scene_card::scene_card(data, assets, &bg.layout(), text)
}

/// A title card (`Scene_MissionTitle(text, 3Fh, 0, 0)` `04EC:5A89`): E1's text card.
pub fn title_card(assets: &StoryAssets, text: &str) -> StoryScreen {
    crate::story::text_card(assets, text)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn marauder_and_hyde_follow_the_e1_routines() {
        let m = Bg::Mar.layout();
        assert_eq!(
            (m.captions, m.shadow, m.down),
            (Captions::Joined((10, 10)), Shadow::L, 15)
        );
        let h = Bg::Hyd.layout();
        assert_eq!(
            (h.captions, h.shadow),
            (Captions::Centred([5, 15, 25]), Shadow::O4)
        );
        assert_eq!(Bg::Vvw.layout().ink, (3, None));
    }
}
