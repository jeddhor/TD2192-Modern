//! The Episode 1 campaign sequencer (`Main_CampaignLoop`): one mission block per call, from
//! the pre-flight story through the flight, retries, debrief, post-flight story and endings.
//!
//! Spec: `docs/spec/flow/episode-program-flow.md` (rev 2) §4.3–§4.5 (the block, the per-mission
//! table and the endings), `docs/spec/flow/screens.md` (rev 1) §3.7 (retry prompt) and §3.12
//! (skip tests); music cues per `docs/spec/audio/music-cues.md` (rev 1) §4.1–§4.2.
//! Fidelity: T2 for the order of screens, T1 for the state handed back to the menu and for the
//! retry prompt's `Random(10)`.
//!
//! The flight itself is not part of this crate: a [`FlightLauncher`] supplied by the program
//! starts it and reports a [`FlightResult`]. [`PlaceholderLauncher`] stands in until the real
//! flight is connected.
//!
//! The skip flag (screens §3.12, U-125): the sequencer tests Esc after most steps. A set flag
//! after a tested pre-flight step jumps to the flight itself (past any music change and hint
//! left); after a tested post-flight step it ends the block (the mission still counts as
//! completed). After an untested step ([`Block::untested`]) the story goes on, so Esc in
//! dialogue 1 still shows hint 1 (C-0002 frames 2603 → 2788).

use std::fmt;

use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_render::small_text;

use crate::dialogue::DialogueScene;
use crate::program::Program;
use crate::screen::{FlowContext, Screen, Transition};
use crate::story::{self, StoryScreen};

/// Backgrounds of the scene cards (§3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bg {
    Hangar,
    Vlab,
    Intro2,
    Shooting,
    Vhang,
    Marauder,
    Briefing,
    Tdlab,
    Velsroom,
    Helihang,
    Eggbase,
    Soffice,
    Sbar,
    Voffice,
    /// `pic4.td` card (`051A:4F5A`).
    Pic4,
    /// `td1-18.pic` with captions (`051A:471B`).
    Td1_18,
}

impl Bg {
    /// The `.scr` file of a full-screen background.
    pub fn file(self) -> Option<&'static str> {
        Some(match self {
            Bg::Hangar => "hangar.scr",
            Bg::Vlab => "vlab.scr",
            Bg::Intro2 => "intro2.scr",
            Bg::Shooting => "shooting.scr",
            Bg::Vhang => "vhang.scr",
            Bg::Marauder => "marauder.scr",
            Bg::Briefing => "briefing.scr",
            Bg::Tdlab => "tdlab.scr",
            Bg::Velsroom => "velsroom.scr",
            Bg::Helihang => "helihang.scr",
            Bg::Eggbase => "eggbase.scr",
            Bg::Soffice => "soffice.scr",
            Bg::Sbar => "sbar.scr",
            Bg::Voffice => "voffice.scr",
            Bg::Pic4 | Bg::Td1_18 => return None,
        })
    }
}

/// One step of a mission block (§4.4 notation). Strings are `0000:OOOO` references into
/// `file0001.exe`, resolved through the resource maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `T[a]`: text card.
    T(u16),
    /// `Mn`: music cue n; `M0` stops the music.
    M(u8),
    /// `C:bg[a/b/c]`: scene card.
    C(Bg, [u16; 3]),
    /// `Dn`: dialogue scene.
    D(u8),
    /// `Hn`: pre-flight hint screen.
    H(u8),
    /// `O`: assignment orders.
    O,
    /// `A`: enemy-ace dossier.
    A,
    /// `Pn[...]`: story-still set n.
    P(u8, [u16; 3]),
    /// `P15`: `td1-15.pic`.
    P15,
    /// Dialogue chosen by the flight outcome: the first when outcome = 1.
    DOutcome(u8, u8),
    /// No skip test between the steps on either side (§4.4, missions 10 and 19).
    Glue,
    /// Flight 21: no retry, no debrief; an abort ends the block (§4.4, mission 19).
    Fly21,
    /// Ending B (mission 17 failure).
    EndingB,
}

/// When the retry prompt is offered (§4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// When the player died.
    Died,
    /// When the player died or the outcome is 0 (mission 17).
    DiedOrOutcome0,
    /// When the player died or the mission was lost; declined while lost → ending C (18, 20).
    DiedOrLost,
}

/// One mission block.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub pre: &'static [Step],
    /// Pre-flight steps with no skip test after them (screens §3.12).
    pub untested: &'static [Step],
    pub retry: Retry,
    pub post: &'static [Step],
    /// Post-flight steps when the outcome is 0, if they differ (mission 17).
    pub post_failed: Option<&'static [Step]>,
}

use Bg::*;
use Step::*;

const H3: Bg = Hangar;
const SO: Bg = Soffice;
const VO: Bg = Voffice;

/// The 20 Episode 1 blocks, transcribed from episode-program-flow §4.4.
pub const E1_BLOCKS: [Block; 20] = [
    Block {
        pre: &[
            T(0x478),
            M(3),
            C(H3, [0x494, 0x4A1, 0x4C1]),
            D(1),
            M(8),
            H(1),
        ],
        untested: &[D(1), H(1)],
        retry: Retry::Died,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x4D0]),
            D(2),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x4F9]),
            D(3),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x50A),
            M(6),
            C(Pic4, [0x52A, 0x542, 0x549]),
            D(4),
            O,
            D(5),
            M(8),
            H(2),
        ],
        untested: &[D(5), H(2)],
        retry: Retry::Died,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x552]),
            D(6),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x4D0]),
            D(7),
            M(5),
            T(0x560),
            C(VO, [0x56D, 0x58A, 0x5A4]),
            D(8),
        ],
        post_failed: None,
    },
    Block {
        pre: &[T(0x5A6), M(3), C(H3, [0x494, 0x4A1, 0x5C0]), D(11)],
        untested: &[D(11)],
        retry: Retry::Died,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x552]),
            DOutcome(13, 12),
            M(5),
            T(0x560),
            C(VO, [0x56D, 0x58A, 0x5A4]),
            D(14),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x5C9),
            M(6),
            C(Briefing, [0x5E3, 0x5FC, 0x5A4]),
            D(15),
            M(3),
            C(H3, [0x494, 0x4A1, 0x607]),
            D(16),
        ],
        untested: &[D(16)],
        retry: Retry::Died,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x4D0]),
            D(17),
            M(5),
            T(0x560),
            C(VO, [0x56D, 0x58A, 0x618]),
            D(18),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x625),
            M(13),
            C(Velsroom, [0x645, 0x660, 0x5A4]),
            C(Intro2, [0x67A, 0x69C, 0x5A4]),
            C(Pic4, [0x6BA, 0x6E0, 0x6FC]),
            A,
            D(19),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x4D0]),
            D(20),
            T(0x70B),
            D(21),
            M(3),
            C(H3, [0x494, 0x4A1, 0x618]),
            D(22),
        ],
        untested: &[A, D(22)],
        retry: Retry::Died,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x4D0]),
            DOutcome(156, 157),
            M(5),
            T(0x560),
            C(VO, [0x56D, 0x58A, 0x5A4]),
            D(23),
        ],
        post_failed: None,
    },
    Block {
        pre: &[T(0x719), M(3), C(H3, [0x494, 0x4A1, 0x607]), D(24), H(6)],
        untested: &[D(24), H(6)],
        retry: Retry::Died,
        post: &[M(3), C(H3, [0x494, 0x4A1, 0x733]), D(25)],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x740),
            M(5),
            C(VO, [0x56D, 0x58A, 0x759]),
            D(26),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x768]),
            D(27),
        ],
        untested: &[D(27)],
        retry: Retry::Died,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x4D0]),
            D(28),
            M(5),
            T(0x560),
            C(VO, [0x56D, 0x58A, 0x776]),
            D(29),
            M(13),
            C(Vlab, [0x787, 0x58A, 0x549]),
            D(100),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x79A),
            M(6),
            C(Briefing, [0x5E3, 0x618, 0x5A4]),
            D(30),
            M(3),
            C(H3, [0x494, 0x4A1, 0x7B8]),
            D(31),
        ],
        untested: &[D(31)],
        retry: Retry::Died,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x4D0]),
            D(32),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x607]),
            D(33),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x7C5),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x4D0]),
            D(34),
            M(3),
            C(H3, [0x494, 0x4A1, 0x4D0]),
            D(35),
        ],
        untested: &[D(35)],
        retry: Retry::Died,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x7E3]),
            D(36),
            M(5),
            T(0x560),
            C(VO, [0x56D, 0x58A, 0x7B8]),
            D(37),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x7F5),
            M(9),
            C(Eggbase, [0x810, 0x5A4, 0x5A4]),
            Glue,
            C(VO, [0x56D, 0x58A, 0x776]),
            D(38),
            M(6),
            C(Briefing, [0x5E3, 0x844, 0x5A4]),
            D(39),
        ],
        untested: &[C(Eggbase, [0x810, 0x5A4, 0x5A4]), D(39)],
        retry: Retry::Died,
        post: &[M(3), C(H3, [0x494, 0x4A1, 0x7E3]), D(40)],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x856),
            M(5),
            C(VO, [0x56D, 0x58A, 0x5C0]),
            D(41),
            M(13),
            C(Shooting, [0x872, 0x886, 0x5A4]),
            D(42),
        ],
        untested: &[D(42)],
        retry: Retry::Died,
        post: &[M(5), C(VO, [0x56D, 0x58A, 0x898]), D(43)],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x8AB),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x759]),
            D(44),
            M(13),
            C(Pic4, [0x52A, 0x542, 0x8C7]),
            D(45),
            M(13),
            C(Helihang, [0x8D3, 0x8EA, 0x5A4]),
            D(46),
        ],
        untested: &[D(46)],
        retry: Retry::Died,
        post: &[
            M(6),
            C(Helihang, [0x8D3, 0x8FE, 0x5A4]),
            D(47),
            M(6),
            C(Pic4, [0x52A, 0x542, 0x549]),
            D(48),
            M(11),
            C(Sbar, [0x913, 0x549, 0x5A4]),
            D(49),
            M(0),
            T(0x938),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x5A4]),
            D(50),
            M(9),
            T(0x5FC),
            C(Velsroom, [0x94B, 0x5A4, 0x5A4]),
            P(3, [0x96F, 0x989, 0x9A5]),
            C(VO, [0x56D, 0x58A, 0x4D0]),
            D(51),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0x9B2),
            M(5),
            C(VO, [0x56D, 0x58A, 0x9D7]),
            D(52),
            M(13),
            C(Vhang, [0x9E8, 0x58A, 0x9F7]),
            D(53),
        ],
        untested: &[D(53)],
        retry: Retry::Died,
        post: &[
            M(13),
            C(Vhang, [0x9E8, 0x58A, 0xA0C]),
            D(54),
            M(5),
            C(VO, [0x56D, 0x58A, 0x898]),
            D(55),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0xA1D),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x5C0]),
            D(56),
            M(6),
            C(Vlab, [0x787, 0x58A, 0x549]),
            D(57),
            M(8),
            C(Intro2, [0xA39, 0xA5B, 0xA74]),
            D(58),
            C(Intro2, [0xA8A, 0xAB3, 0xAD0]),
            M(13),
            C(Vlab, [0x787, 0x58A, 0xAE1]),
            D(59),
            M(9),
            T(0x9F7),
            C(VO, [0x56D, 0x58A, 0x5A4]),
            D(60),
        ],
        untested: &[D(60)],
        retry: Retry::Died,
        post: &[
            M(4),
            C(SO, [0x4E0, 0x4A1, 0xAF2]),
            D(61),
            M(6),
            C(Tdlab, [0xB08, 0xB1A, 0x5A4]),
            D(108),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0xB2A),
            M(5),
            C(VO, [0x56D, 0x58A, 0x759]),
            D(62),
            M(4),
            C(SO, [0x4E0, 0x4A1, 0x776]),
            D(63),
            M(6),
            C(Briefing, [0x5E3, 0xB49, 0x5A4]),
            D(64),
            M(13),
            C(Helihang, [0xB5A, 0x898, 0x5A4]),
            D(109),
        ],
        untested: &[D(109)],
        retry: Retry::Died,
        post: &[
            M(13),
            C(Helihang, [0xB5A, 0x8FE, 0x5A4]),
            D(65),
            M(11),
            C(Sbar, [0x913, 0x549, 0x5A4]),
            D(66),
            M(9),
            T(0x5FC),
            C(SO, [0x4E0, 0x4A1, 0x5A4]),
            D(67),
            P(4, [0xB6C, 0xB88, 0xBBE]),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0xBC1),
            M(13),
            C(Vlab, [0x787, 0x58A, 0x5C0]),
            D(68),
            M(5),
            C(VO, [0x56D, 0x58A, 0xBDB]),
            D(69),
            M(12),
            P(1, [0xBF5, 0xC16, 0x5A4]),
            M(4),
            C(SO, [0xC45, 0x4A1, 0xC64]),
            D(70),
            M(3),
            C(H3, [0x494, 0x4A1, 0x4F9]),
            D(71),
        ],
        untested: &[D(71)],
        retry: Retry::Died,
        post: &[M(5), C(VO, [0x56D, 0x58A, 0x898]), DOutcome(72, 105)],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0xC74),
            M(4),
            C(SO, [0xC45, 0x4A1, 0x618]),
            D(73),
            M(3),
            C(H3, [0x494, 0x4A1, 0xC91]),
            D(74),
        ],
        untested: &[D(74)],
        retry: Retry::DiedOrOutcome0,
        post: &[
            M(11),
            C(Sbar, [0x913, 0x549, 0x5A4]),
            D(76),
            M(5),
            C(VO, [0x56D, 0x58A, 0x898]),
            D(77),
        ],
        post_failed: Some(&[M(5), C(VO, [0x56D, 0x58A, 0x898]), D(75), EndingB]),
    },
    Block {
        pre: &[
            T(0xCA5),
            M(5),
            C(VO, [0x56D, 0x58A, 0x898]),
            D(78),
            M(3),
            C(H3, [0x494, 0x4A1, 0xCBF]),
            D(80),
            M(4),
            C(SO, [0xC45, 0x4A1, 0x607]),
            D(81),
        ],
        untested: &[D(81)],
        retry: Retry::DiedOrLost,
        post: &[
            M(3),
            C(H3, [0x494, 0x4A1, 0x7E3]),
            D(82),
            M(5),
            T(0x560),
            C(VO, [0x56D, 0x58A, 0x5A4]),
            D(83),
            P15,
            D(84),
            M(13),
            C(Vlab, [0x560, 0xCCD, 0x5A4]),
            C(Td1_18, [0xCE7, 0xCFC, 0x5A4]),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0xD27),
            M(6),
            C(Briefing, [0x5E3, 0xD47, 0x5A4]),
            D(85),
            M(4),
            C(SO, [0xC45, 0x4A1, 0x898]),
            Glue,
            D(86),
            M(3),
            C(H3, [0x494, 0x4A1, 0xAF2]),
            D(87),
        ],
        untested: &[C(SO, [0xC45, 0x4A1, 0x898]), D(87)],
        retry: Retry::Died,
        post: &[
            M(5),
            C(VO, [0x56D, 0x58A, 0x898]),
            D(88),
            M(3),
            C(H3, [0x494, 0x4A1, 0x7E3]),
            D(89),
            Fly21,
            M(13),
            T(0xD54),
            C(Tdlab, [0xD69, 0xD89, 0x5A4]),
            D(91),
            M(5),
            T(0x560),
            C(Marauder, [0xD98, 0xDB0, 0xDC7]),
            D(90),
        ],
        post_failed: None,
    },
    Block {
        pre: &[
            T(0xDD3),
            M(4),
            C(SO, [0xC45, 0x4A1, 0xDED]),
            D(92),
            M(11),
            C(Sbar, [0x913, 0x549, 0x5A4]),
            D(93),
            M(4),
            C(SO, [0xC45, 0x4A1, 0xDFE]),
            D(110),
            M(13),
            C(Tdlab, [0xE10, 0xE20, 0x5A4]),
            D(94),
            M(3),
            C(H3, [0x494, 0x4A1, 0x5C0]),
            D(95),
        ],
        untested: &[D(95)],
        retry: Retry::DiedOrLost,
        post: &[
            M(13),
            C(Tdlab, [0xE10, 0x552, 0x5A4]),
            D(96),
            M(4),
            C(SO, [0xC45, 0x4A1, 0x4D0]),
            D(97),
            M(6),
            C(Tdlab, [0xE10, 0x549, 0x5A4]),
            D(98),
            M(9),
            C(Velsroom, [0xE37, 0x7B8, 0x5A4]),
            D(99),
            P(2, [0x5A4, 0x5A4, 0x5A4]),
            T(0xE51),
        ],
        post_failed: None,
    },
];

/// Ending B and C scene strings (§4.5).
const ENDING_B: [u16; 3] = [0xE6B, 0xE9B, 0xEC3];
const ENDING_C: [u16; 3] = [0xED4, 0xE9B, 0xEC3];

/// What a flight reports when it ends (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlightResult {
    /// `g_PlayerDied`.
    pub died: bool,
    /// `g_MissionAborted`: quit from the flight menu.
    pub aborted: bool,
    /// `g_MissionLost` (missions 17, 18, 20).
    pub lost: bool,
    /// `g_MissionOutcome` (1 by default).
    pub outcome: bool,
    /// `g_MissionKills` of this attempt.
    pub kills: i16,
}

impl Default for FlightResult {
    fn default() -> Self {
        Self {
            died: false,
            aborted: false,
            lost: false,
            outcome: true,
            kills: 0,
        }
    }
}

/// A running flight: a screen that pops itself when the flight ends.
pub trait FlightScreen: Screen {
    /// The result, read after the screen popped.
    fn result(&self) -> FlightResult;
}

/// Starts flights for the campaign (`Sim_FlyMission(n)`, n = 1..=21).
///
/// A launcher lives for one program run and owns what the original keeps across its flights,
/// above all the program's `Random` generator: the flights and the front end draw from one
/// stream (bp7-runtime-semantics; the retry prompt's `Random(10)`, screens.md U-124).
pub trait FlightLauncher: fmt::Debug {
    fn launch(&mut self, ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen>;

    /// `Random(n)` from the program's generator (the stream the flights use).
    fn random(&mut self, n: u16) -> u16;
}

/// Stand-in flight: a notice; any key = success, D = died, A = aborted, L = lost,
/// O = outcome 0.
#[derive(Debug, Default)]
pub struct PlaceholderLauncher {
    /// The program's generator, started as a real launcher starts it (program start
    /// included) on first use.
    rng: Option<td_sim::BpRandom>,
}

impl FlightLauncher for PlaceholderLauncher {
    fn random(&mut self, n: u16) -> u16 {
        self.rng
            .get_or_insert_with(|| td_sim::flight::Session::new().rng)
            .random(n)
    }

    fn launch(&mut self, ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        let font = ctx.data.read("td.lt2").ok().and_then(|b| {
            td_formats::font::Font::parse(&b, td_formats::font::FontKind::Small).ok()
        });
        Box::new(PlaceholderFlight {
            flight,
            font,
            result: None,
        })
    }
}

#[derive(Debug)]
struct PlaceholderFlight {
    flight: u8,
    font: Option<td_formats::font::Font>,
    result: Option<FlightResult>,
}

impl Screen for PlaceholderFlight {
    fn tick(&mut self, _ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        let Some(&k) = input.pressed.iter().find(|&&k| story::is_keystroke(k)) else {
            return Transition::None;
        };
        let mut r = FlightResult::default();
        match k {
            k if Some(k) == Key::letter('d') => r.died = true,
            k if Some(k) == Key::letter('a') => r.aborted = true,
            k if Some(k) == Key::letter('l') => r.lost = true,
            k if Some(k) == Key::letter('o') => r.outcome = false,
            _ => r.kills = 3,
        }
        self.result = Some(r);
        Transition::Pop
    }

    fn draw(&self, fb: &mut Framebuffer) {
        fb.clear(0);
        fb.palette.colors[15] = td_formats::palette::Rgb6 {
            r: 63,
            g: 63,
            b: 63,
        };
        let lines = [
            format!("flight {} would start here", self.flight),
            "any key - mission completed".to_owned(),
            "d - died   a - aborted   l - lost   o - outcome 0".to_owned(),
        ];
        if let Some(font) = &self.font {
            for (i, l) in lines.iter().enumerate() {
                small_text::draw(fb, font, l, (20, 80 + 10 * i as i32), 15);
            }
        }
    }
}

impl FlightScreen for PlaceholderFlight {
    fn result(&self) -> FlightResult {
        self.result.unwrap_or_default()
    }
}

/// The debrief rating (§4.5): the last matching tier for T = kills to date + mission kills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rating {
    Cheater,
    /// Tier 1..=11 (`debrief.tiers` index + 1).
    Tier(u8),
}

/// Rating tier of a total (§4.5; test vector 3).
pub fn rating(total: i32) -> Rating {
    if !(0..=29_999).contains(&total) {
        return Rating::Cheater;
    }
    let tier = [9, 19, 39, 59, 79, 109, 149, 174, 199, 299]
        .iter()
        .filter(|&&t| total > t)
        .count();
    Rating::Tier(tier as u8 + 1)
}

/// What a block hands back to the menu (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockResult {
    /// `g_MissionAdvance`: the mission counts as completed.
    pub advance: bool,
    /// `g_GameOver`.
    pub game_over: bool,
    /// Kills of the last flight.
    pub mission_kills: i16,
}

/// Where the block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pos {
    Pre(usize),
    Fly,
    Retry,
    Debrief,
    Post(usize),
    Ending(usize),
    Return,
}

enum Child {
    Story(Box<StoryScreen>),
    Dialogue(Box<DialogueScene>),
    Flight(Box<dyn FlightScreen>),
}

/// One mission block in progress.
pub struct Campaign {
    mission: u8,
    kills_to_date: i16,
    block: Block,
    pos: Pos,
    child: Option<Child>,
    /// The step the child belongs to.
    child_step: Option<Step>,
    last: FlightResult,
    attempts: u32,
    ending: &'static [Step],
    ending_dossier: bool,
    resume_after_fly21: Pos,
    result: BlockResult,
}

impl fmt::Debug for Campaign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Campaign")
            .field("mission", &self.mission)
            .field("pos", &self.pos)
            .finish_non_exhaustive()
    }
}

const ENDING_B_STEPS: &[Step] = &[M(12), C(Intro2, ENDING_B)];
const ENDING_C_STEPS: &[Step] = &[M(12), C(Intro2, ENDING_C)];

impl Campaign {
    /// A block for `mission` (1..=20) with `kills_to_date` before it.
    ///
    /// # Panics
    /// If `mission` is not 1..=20.
    pub fn new(mission: u8, kills_to_date: i16) -> Self {
        assert!((1..=20).contains(&mission), "mission {mission}");
        Self {
            mission,
            kills_to_date,
            block: E1_BLOCKS[usize::from(mission) - 1],
            pos: Pos::Pre(0),
            child: None,
            child_step: None,
            last: FlightResult::default(),
            attempts: 0,
            ending: &[],
            ending_dossier: false,
            resume_after_fly21: Pos::Return,
            result: BlockResult {
                advance: true,
                ..BlockResult::default()
            },
        }
    }

    /// True while the retry prompt is shown.
    pub fn in_retry_prompt(&self) -> bool {
        self.pos == Pos::Retry
    }

    /// Runs the block for one frame; returns the result when the block is over.
    pub fn step(
        &mut self,
        prog: &mut Program,
        ctx: &mut FlowContext<'_>,
        input: &TickInput,
    ) -> Option<BlockResult> {
        let mut input = Some(input);
        loop {
            if let Some(child) = &mut self.child {
                let inp = input.take()?;
                let done = match child {
                    Child::Story(s) => {
                        s.step(inp);
                        s.finished()
                    }
                    Child::Dialogue(d) => {
                        d.step(inp);
                        d.finished()
                    }
                    Child::Flight(f) => matches!(f.tick(ctx, inp), Transition::Pop),
                };
                if !done {
                    return None;
                }
                let child = self.child.take();
                self.finish_child(prog, ctx, child);
                continue;
            }
            if self.pos == Pos::Return {
                return Some(self.result);
            }
            self.advance(prog, ctx);
        }
    }

    /// Draws the current child (black between screens).
    pub fn draw(&self, fb: &mut Framebuffer) {
        match &self.child {
            Some(Child::Story(s)) => s.draw(fb),
            Some(Child::Dialogue(d)) => d.draw(fb),
            Some(Child::Flight(f)) => f.draw(fb),
            None => {
                fb.clear(0);
                fb.palette = Default::default();
            }
        }
    }

    /// A child finished: read its skip flag or result and move on.
    fn finish_child(
        &mut self,
        prog: &mut Program,
        ctx: &mut FlowContext<'_>,
        child: Option<Child>,
    ) {
        let skipped = match &child {
            Some(Child::Story(s)) => s.skipped(),
            Some(Child::Dialogue(d)) => d.skipped(),
            _ => false,
        };
        let step = self.child_step.take();
        match (self.pos, child) {
            (Pos::Fly, Some(Child::Flight(f))) if step == Some(Fly21) => {
                prog.cues.stop(ctx.audio);
                let r = f.result();
                if r.aborted {
                    self.result.advance = false;
                    self.pos = Pos::Return;
                } else {
                    self.pos = self.resume_after_fly21;
                }
            }
            (Pos::Fly, Some(Child::Flight(f))) => {
                prog.cues.stop(ctx.audio);
                self.last = f.result();
                self.result.mission_kills = self.last.kills;
                let r = self.last;
                let offered = r.died
                    || match self.block.retry {
                        Retry::Died => false,
                        Retry::DiedOrOutcome0 => !r.outcome,
                        Retry::DiedOrLost => r.lost,
                    };
                self.pos = if offered {
                    self.child = Some(Child::Story(Box::new(prog.retry_prompt())));
                    Pos::Retry
                } else {
                    self.resolve()
                };
            }
            (Pos::Retry, Some(Child::Story(s))) => {
                self.pos = if s.last_key() == Some(Key::ESCAPE) {
                    self.resolve()
                } else {
                    Pos::Fly
                };
            }
            (Pos::Debrief, _) => self.pos = Pos::Post(0),
            (Pos::Pre(i), _) => {
                let glued = self.block.pre.get(i + 1) == Some(&Glue);
                let tested = step.is_some_and(|s| !self.block.untested.contains(&s));
                self.pos = if skipped && tested && !glued {
                    Pos::Fly
                } else {
                    Pos::Pre(i + 1)
                };
            }
            (Pos::Post(i), _) => {
                let glued = self.post().get(i + 1) == Some(&Glue);
                // Untested post-flight steps (screens §3.12): the outcome branches, block 18's
                // P15 and block 19's D89.
                let tested = !matches!(step, Some(DOutcome(..) | P15 | D(89)));
                self.pos = if skipped && tested && !glued {
                    Pos::Return
                } else {
                    Pos::Post(i + 1)
                };
            }
            (Pos::Ending(i), _) => self.pos = Pos::Ending(i + 1),
            (pos, _) => self.pos = pos,
        }
    }

    /// After a declined retry or a flight without one (§4.3).
    fn resolve(&mut self) -> Pos {
        let r = self.last;
        if r.lost && self.block.retry == Retry::DiedOrLost {
            return self.game_over(ENDING_C_STEPS, false);
        }
        if r.died {
            return self.game_over(&[M(12)], true);
        }
        if r.aborted {
            self.result.advance = false;
            return Pos::Return;
        }
        Pos::Debrief
    }

    fn game_over(&mut self, steps: &'static [Step], dossier: bool) -> Pos {
        self.ending = steps;
        self.ending_dossier = dossier;
        self.result.game_over = true;
        self.result.advance = false;
        Pos::Ending(0)
    }

    fn post(&self) -> &'static [Step] {
        match self.block.post_failed {
            Some(steps) if !self.last.outcome => steps,
            _ => self.block.post,
        }
    }

    /// Starts whatever comes next at the current position.
    fn advance(&mut self, prog: &mut Program, ctx: &mut FlowContext<'_>) {
        let step = match self.pos {
            Pos::Pre(i) => match self.block.pre.get(i) {
                Some(s) => *s,
                None => {
                    self.pos = Pos::Fly;
                    return;
                }
            },
            Pos::Fly => {
                self.attempts += 1;
                self.launch(prog, ctx, self.mission);
                return;
            }
            Pos::Debrief => {
                prog.play_cue(ctx, 8);
                self.child = Some(Child::Story(Box::new(self.debrief(prog))));
                return;
            }
            Pos::Post(i) => match self.post().get(i) {
                Some(s) => *s,
                None => {
                    self.pos = Pos::Return;
                    return;
                }
            },
            Pos::Ending(i) => match self.ending.get(i) {
                Some(s) => *s,
                None if self.ending_dossier && i == self.ending.len() => {
                    // Ending A: the pilot dossier, then the loop flag is restored.
                    let total = self.kills_to_date.saturating_add(self.last.kills);
                    self.child = Some(Child::Story(Box::new(prog.player_dossier(total))));
                    self.child_step = None;
                    return;
                }
                None => {
                    prog.cues.set_loop(true, ctx.audio);
                    self.pos = Pos::Return;
                    return;
                }
            },
            Pos::Retry | Pos::Return => return,
        };
        self.child_step = Some(step);
        match step {
            M(0) => prog.cues.stop(ctx.audio),
            M(n) => prog.play_cue(ctx, n),
            Glue => {}
            Fly21 => {
                self.resume_after_fly21 = Pos::Post(self.post_index() + 1);
                self.pos = Pos::Fly;
                self.launch(prog, ctx, 21);
                self.child_step = Some(Fly21);
                return;
            }
            EndingB => {
                self.pos = self.game_over(ENDING_B_STEPS, false);
                return;
            }
            DOutcome(a, b) => {
                let id = if self.last.outcome { a } else { b };
                self.child = Some(Child::Dialogue(Box::new(prog.dialogue(id))));
            }
            D(id) => self.child = Some(Child::Dialogue(Box::new(prog.dialogue(id)))),
            other => match prog.story(other) {
                Ok(s) => self.child = Some(Child::Story(Box::new(s))),
                Err(e) => tracing::warn!("story step {other:?} unavailable: {e}"),
            },
        }
        if self.child.is_none() {
            // Instant steps (music, glue, unavailable screens) move on at once.
            self.finish_child(prog, ctx, None);
        }
    }

    fn post_index(&self) -> usize {
        match self.pos {
            Pos::Post(i) => i,
            _ => 0,
        }
    }

    fn launch(&mut self, prog: &mut Program, ctx: &mut FlowContext<'_>, flight: u8) {
        prog.cues.stop(ctx.audio);
        prog.play_flight_song(ctx, flight);
        let f = prog.launcher.launch(ctx, flight);
        self.child = Some(Child::Flight(f));
    }

    fn debrief(&self, prog: &Program) -> StoryScreen {
        prog.debrief(
            self.mission,
            self.last.kills,
            self.kills_to_date,
            self.last.outcome,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debrief_tiers() {
        // episode-program-flow §6 vector 3.
        for (t, r) in [
            (0, Rating::Tier(1)),
            (9, Rating::Tier(1)),
            (10, Rating::Tier(2)),
            (20, Rating::Tier(3)),
            (40, Rating::Tier(4)),
            (60, Rating::Tier(5)),
            (80, Rating::Tier(6)),
            (110, Rating::Tier(7)),
            (150, Rating::Tier(8)),
            (175, Rating::Tier(9)),
            (200, Rating::Tier(10)),
            (300, Rating::Tier(11)),
            (29_999, Rating::Tier(11)),
            (30_000, Rating::Cheater),
            (-1, Rating::Cheater),
        ] {
            assert_eq!(rating(t), r, "{t}");
        }
    }

    #[test]
    fn every_dialogue_scene_is_used_once() {
        // §9: all 103 scene IDs of dialogue.dat are referenced exactly once.
        let mut ids = Vec::new();
        for b in &E1_BLOCKS {
            for s in b
                .pre
                .iter()
                .chain(b.post)
                .chain(b.post_failed.unwrap_or(&[]))
            {
                match *s {
                    D(n) => ids.push(n),
                    DOutcome(a, c) => ids.extend([a, c]),
                    _ => {}
                }
            }
        }
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "no scene twice");
        assert_eq!(n, 103);
        assert!(
            !ids.contains(&9) && !ids.contains(&10),
            "scenes 9 and 10 do not exist"
        );
    }
}
