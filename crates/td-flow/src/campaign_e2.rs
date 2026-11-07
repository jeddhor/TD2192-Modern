//! The Episode 2 campaign sequencer (`td2.exe` `Main_CampaignLoop` `0000:0CC3`): one mission
//! block per call, from the pre-flight story through the flight, its retries, the debrief, the
//! outcome rules and the post-flight story, and the three endings.
//!
//! Spec: `docs/spec/flow/e2-program-flow.md` rev 1 §3.4 (flight hand-off), §4.3 (one block),
//! §4.4 (the 20 blocks, transcribed into [`E2_BLOCKS`]), §4.5 (endings), §4.6 (bookkeeping);
//! scene cards per [`crate::scenes_e2`]; the shared screens (dialogue, retry prompt, debrief,
//! pilot dossier) come from [`Program`] with `episode = E2`. Fidelity: T2 for the order of
//! screens, T1 for the state handed back to the menu and for the retry prompt's `Random(10)`.
//!
//! Skip tests (§4.4): every story step is followed by a skip test except the steps marked `'`
//! in the spec ([`Step::Nt`] after them here). A set flag after a tested pre-flight step jumps
//! to the flight; after a tested post-flight step it ends the block (the mission still counts).
//! Music cues are instant and carry no test, as in the E1 sequencer.
//!
//! Every screen updates the front-end state of the program run ([`Program::front_end`]) when it
//! ends: its `g_Palette` writes ([`Child::apply_front_end`]) and the last name it assigned to
//! `g_SceneFile` ([`Child::scene_file`]); the launcher receives the state before each flight
//! ([`crate::FlightLauncher::set_front_end`]), whose two-digit-heading volleys read it (weapons
//! rev 5 §3.3.2, OB-017).

use std::fmt;

use td_assets::Episode;
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_sim::flight::front_end::{FrontEnd, SCENE_FILE_CARDS, SCENE_FILE_PALETTE};

use crate::campaign::{FlightResult, FlightScreen};
use crate::dialogue::DialogueScene;
use crate::program::{Program, line};
use crate::scenes_e2::{self, Bg};
use crate::screen::{FlowContext, RETRACE_HZ, Screen, Transition};
use crate::story::{StoryError, StoryScreen};

/// A caption (e2-program-flow §2 notation; the same in E3, e3-program-flow §2, with `e3.` ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    /// `m.k`: `e2.mission.MM.cards[k]` (E3: `e3.mission.MM.cards[k]`).
    C(u8, u8),
    /// `p.k`: `e2.story.preamble[k]` (E3: `e3.story.preamble[k]`).
    P(u8),
}

/// `_`: `e2.mission.01.cards[2]`, a single space.
pub const BLANK: Cap = Cap::C(1, 2);

impl Cap {
    /// The resource list and index (Episode 2).
    pub fn resource(self) -> (String, usize) {
        self.resource_in(Episode::E2)
    }

    /// The resource list and index in `episode` (E2 or E3).
    pub fn resource_in(self, episode: Episode) -> (String, usize) {
        let e = if episode == Episode::E3 { "e3" } else { "e2" };
        match self {
            Cap::C(m, k) => (format!("{e}.mission.{m:02}.cards"), usize::from(k)),
            Cap::P(k) => (format!("{e}.story.preamble"), usize::from(k)),
        }
    }
}

/// One step of a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `Mn`: music cue n (E2 table, `td_audio::CueTable::E2`).
    M(u8),
    /// `Dn`: dialogue scene n.
    D(u8),
    /// `Tnn`: the title card of mission nn.
    T(u8),
    /// `T(m.k)`: a title-style card with a caption.
    Tc(Cap),
    /// `bg[a/b/c]`: a scene card.
    S(Bg, [Cap; 3]),
    /// `'`: the step before has no skip test.
    Nt,
    /// Block 3 after the debrief: outcome 1 → D14 (tested); outcome 0 → D247 (untested), then
    /// the retry prompt (accept → fly again, decline → ending A).
    Outcome3,
    /// Endings: `WORX_SetMusicLoop(0)` / `(1)`.
    LoopOff,
    LoopOn,
    /// Ending A: the pilot dossier (kills to date + mission kills).
    Dossier,
}

/// What follows the debrief when the outcome is 0 (§4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeRule {
    None,
    /// Blocks 8, 17: retry prompt; decline → mission −= 1 and return (the block is offered
    /// again, OB-027).
    Replay,
    /// Block 16: retry prompt; decline → ending B (unreachable, OB-025/OB-042).
    EndingB,
}

/// One mission block.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub pre: &'static [Step],
    /// Blocks 17 and 18: the lost flag has its own retry prompt and ending C.
    pub lost_rules: bool,
    pub outcome: OutcomeRule,
    pub post: &'static [Step],
}

use Bg::*;
use Cap::{C, P};
use Step::*;

const PRE: [Cap; 3] = [P(0), P(1), P(2)];
const B: Cap = BLANK;

const fn std(pre: &'static [Step], post: &'static [Step]) -> Block {
    Block {
        pre,
        lost_rules: false,
        outcome: OutcomeRule::None,
        post,
    }
}

/// The 20 Episode 2 blocks, transcribed from e2-program-flow rev 1 §4.4.
pub const E2_BLOCKS: [Block; 20] = [
    std(
        &[
            M(5),
            S(Mar, PRE),
            D(1),
            T(1),
            M(10),
            S(Lab, [C(1, 0), C(1, 1), B]),
            D(2),
            M(4),
            S(Sof, [C(1, 3), C(1, 4), C(1, 5)]),
            D(3),
            M(3),
            S(Hg, [C(1, 6), C(1, 4), C(1, 7)]),
            D(4),
        ],
        &[
            M(3),
            S(Hg, [C(1, 6), C(1, 4), C(1, 8)]),
            D(5),
            M(13),
            S(Bun, [C(1, 9), C(1, 10), B]),
            D(6),
        ],
    ),
    std(
        &[
            T(2),
            M(6),
            S(Vel, [C(2, 0), C(2, 1), B]),
            D(7),
            M(10),
            S(Brf, [C(2, 2), C(2, 3), B]),
            D(8),
            M(3),
            S(Hg, [C(1, 6), C(1, 4), C(2, 4)]),
            D(130),
        ],
        &[
            M(3),
            S(Hg, [C(1, 6), C(1, 4), C(1, 8)]),
            D(131),
            M(11),
            S(Bar, [C(2, 5), C(2, 6), C(2, 3)]),
            Nt,
            D(11),
        ],
    ),
    std(
        &[
            M(6),
            S(Sho, [C(2, 7), C(2, 8), B]),
            D(12),
            T(3),
            M(10),
            S(Brf, [C(2, 2), C(1, 7), B]),
            D(233),
        ],
        &[M(6), Outcome3, M(13), S(Bun, [C(1, 9), C(1, 10), B]), D(15)],
    ),
    std(
        &[
            T(4),
            M(10),
            S(Lab, [C(1, 0), C(4, 0), B]),
            D(16),
            M(4),
            S(Sof, [C(1, 3), C(1, 4), C(1, 5)]),
            D(17),
            M(3),
            S(Hg, [C(1, 6), C(1, 4), C(4, 1)]),
            D(18),
        ],
        &[
            M(3),
            S(Hg, [C(1, 6), C(1, 4), C(1, 7)]),
            D(19),
            M(13),
            S(Sew, [C(4, 2), C(4, 3), B]),
            D(20),
            M(4),
            S(Sof, [C(1, 3), C(1, 4), C(4, 4)]),
            D(21),
            M(10),
            S(Bar, [C(2, 5), C(2, 6), C(2, 3)]),
            D(22),
        ],
    ),
    std(
        &[
            M(13),
            S(Bun, [C(1, 9), C(4, 5), B]),
            D(23),
            T(5),
            M(10),
            S(Sew, [C(4, 2), C(4, 3), B]),
            D(24),
        ],
        &[
            M(13),
            S(Sew, [C(4, 2), C(5, 0), B]),
            D(25),
            M(10),
            S(Bun, [C(1, 9), C(5, 1), B]),
            D(26),
            M(6),
            S(Sew, [C(4, 2), C(5, 0), B]),
            D(27),
        ],
    ),
    std(
        &[
            M(13),
            S(Sew, [C(4, 2), C(5, 2), C(5, 3)]),
            D(28),
            T(6),
            M(5),
            S(Mar, PRE),
            D(29),
            M(6),
            S(Vid, [C(6, 0), C(1, 1), B]),
            D(30),
            M(10),
            S(Sew, [C(4, 2), C(1, 7), B]),
            D(31),
        ],
        &[M(10), S(Bun, [C(1, 9), C(5, 0), B]), D(32)],
    ),
    std(
        &[
            T(7),
            M(13),
            S(Bun, [C(1, 9), C(1, 1), B]),
            D(34),
            S(Vvw, [B, B, B]),
            Nt,
            D(35),
            Nt,
            M(10),
            S(Elv, [C(7, 0), B, B]),
            D(236),
        ],
        &[
            M(3),
            S(Hg, [C(1, 6), C(1, 4), C(1, 7)]),
            D(36),
            M(4),
            S(Sof, [C(1, 3), C(1, 4), C(2, 4)]),
            D(37),
            M(5),
            S(Mar, PRE),
            D(38),
        ],
    ),
    Block {
        pre: &[
            T(8),
            M(10),
            S(Lab, [C(1, 0), C(1, 1), B]),
            D(39),
            M(5),
            S(Mar, PRE),
            D(40),
            M(6),
            S(Brf, [C(2, 2), C(2, 1), B]),
            D(41),
            M(11),
            S(Bar, [C(2, 5), C(2, 6), C(2, 3)]),
            D(42),
            M(5),
            S(Mar, [C(8, 0), P(1), P(2)]),
            D(46),
            S(Grn, [C(8, 1), C(8, 2), C(8, 3)]),
            M(3),
            S(Hg, [C(1, 6), C(1, 4), B]),
            Nt,
            D(47),
        ],
        lost_rules: false,
        outcome: OutcomeRule::Replay,
        post: &[
            M(11),
            S(Heli, [C(8, 4), C(8, 5), C(1, 7)]),
            D(48),
            M(6),
            D(49),
            M(10),
            S(To2, [C(8, 6), C(8, 7), C(1, 1)]),
            D(50),
        ],
    },
    std(
        &[
            T(9),
            M(6),
            S(To2, [C(8, 6), C(8, 7), C(2, 1)]),
            D(51),
            M(10),
            S(Sho, [C(2, 7), C(9, 0), B]),
            D(52),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(4, 1)]),
            D(53),
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(1, 7)]),
            D(54),
            Tc(C(9, 1)),
            D(55),
        ],
        &[
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(1, 8)]),
            D(56),
            M(5),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(57),
        ],
    ),
    std(
        &[
            T(10),
            M(6),
            S(Sho, [C(2, 7), C(5, 3), B]),
            D(58),
            M(5),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(59),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(4, 1)]),
            D(60),
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(1, 7)]),
            D(61),
        ],
        &[
            M(10),
            S(To2, [C(8, 6), C(8, 7), C(4, 1)]),
            D(62),
            M(13),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(63),
        ],
    ),
    std(
        &[
            T(11),
            M(11),
            S(Bar, [C(2, 5), C(11, 0), C(11, 1)]),
            D(64),
            M(3),
            S(Hg2, [C(11, 2), C(8, 5), C(2, 4)]),
            D(65),
            S(Vid, [C(11, 3), C(11, 4), C(11, 5)]),
            Nt,
            D(66),
        ],
        &[
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(1, 8)]),
            D(67),
            M(5),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(68),
        ],
    ),
    std(
        &[
            T(12),
            M(6),
            S(Bar, [C(2, 5), C(11, 0), C(11, 1)]),
            D(69),
            M(10),
            S(Brf, [C(12, 0), C(2, 1), B]),
            D(70),
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(2, 4)]),
            D(71),
        ],
        &[M(4), S(To2, [C(8, 6), C(8, 7), C(4, 1)]), D(72)],
    ),
    std(
        &[
            T(13),
            M(6),
            S(To2, [C(8, 6), C(8, 7), C(4, 1)]),
            D(73),
            M(11),
            S(Bar, [C(2, 5), C(11, 0), C(4, 4)]),
            D(74),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(2, 1)]),
            D(75),
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(2, 4)]),
            D(76),
        ],
        &[
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(1, 7)]),
            D(77),
            M(13),
            S(Lab, [C(1, 0), C(1, 1), B]),
            D(78),
        ],
    ),
    std(
        &[
            T(14),
            M(6),
            S(Bar, [C(2, 5), C(11, 0), C(4, 4)]),
            D(79),
            M(5),
            S(Mar, PRE),
            D(80),
            M(10),
            S(Bar, [C(2, 5), C(11, 0), C(1, 7)]),
            D(81),
            M(6),
            S(To2, [C(8, 6), C(8, 7), C(2, 1)]),
            D(181),
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(2, 4)]),
            D(82),
        ],
        &[
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(14, 0)]),
            D(83),
            M(6),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(84),
        ],
    ),
    std(
        &[
            T(15),
            M(6),
            S(Lab, [C(1, 0), C(1, 1), B]),
            D(85),
            M(10),
            S(Sho, [C(2, 7), C(2, 3), B]),
            D(86),
            Tc(C(9, 1)),
            D(87),
            M(13),
            S(Lab, [C(1, 0), C(1, 7), B]),
            D(187),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(15, 0)]),
            D(88),
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(2, 4)]),
            D(89),
        ],
        &[
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(14, 0)]),
            D(90),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(4, 4)]),
            D(91),
        ],
    ),
    Block {
        pre: &[
            T(16),
            M(5),
            S(Mar, [P(0), P(1), C(16, 0)]),
            D(92),
            M(10),
            S(Lab, [C(16, 1), C(16, 2), B]),
            D(93),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(16, 3)]),
            D(94),
        ],
        lost_rules: false,
        outcome: OutcomeRule::EndingB,
        post: &[
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(14, 0)]),
            D(95),
            M(5),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(96),
        ],
    },
    Block {
        pre: &[
            T(17),
            M(6),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(97),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(17, 0)]),
            D(98),
            M(10),
            S(Vpt, [B, B, B]),
            Nt,
            D(99),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(11, 4)]),
            D(100),
            M(13),
            S(Brf, [C(2, 2), C(1, 7), B]),
            D(101),
        ],
        lost_rules: true,
        outcome: OutcomeRule::Replay,
        post: &[
            M(5),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(102),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(2, 4)]),
            D(103),
            M(5),
            S(Mar, PRE),
            D(104),
        ],
    },
    Block {
        pre: &[
            T(18),
            M(10),
            S(To2, [C(18, 0), C(8, 7), C(4, 0)]),
            D(105),
            M(6),
            S(Vid, [C(18, 1), C(18, 2), B]),
            D(106),
            M(10),
            S(Bar, [C(2, 5), C(11, 0), C(1, 7)]),
            D(107),
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(18, 3)]),
            D(108),
        ],
        lost_rules: true,
        outcome: OutcomeRule::None,
        post: &[
            M(4),
            S(To2, [C(18, 0), C(8, 7), C(9, 0)]),
            D(109),
            M(10),
            S(Vid, [C(18, 4), C(18, 5), B]),
            D(110),
        ],
    },
    std(
        &[
            T(19),
            M(5),
            S(Mar, PRE),
            D(112),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(19, 0)]),
            D(113),
            M(5),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(114),
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(11, 1)]),
            D(115),
        ],
        &[
            M(3),
            S(Hg2, [C(1, 6), C(8, 5), C(1, 8)]),
            D(116),
            M(5),
            S(Hyd, [C(9, 2), C(9, 3), C(1, 1)]),
            D(117),
            S(Bb, [C(19, 1), C(19, 2), B]),
        ],
    ),
    std(
        &[
            T(20),
            M(4),
            S(To2, [C(8, 6), C(8, 7), C(17, 0)]),
            D(119),
            S(Ipt, [B, B, B]),
            Nt,
            M(5),
            S(Mar, PRE),
            D(120),
            M(10),
            S(To2, [C(8, 6), C(8, 7), C(2, 4)]),
            D(121),
            M(6),
            S(Bar, [C(2, 5), C(11, 0), C(4, 4)]),
            D(122),
        ],
        &[
            M(13),
            S(Shu, [C(20, 0), C(20, 1), C(14, 0)]),
            D(123),
            S(Hdd, [B, B, B]),
            D(124),
            M(5),
            S(Mar, [C(8, 0), P(1), P(2)]),
            D(125),
            M(12),
            S(Die, [C(20, 2), C(20, 3), C(20, 4)]),
            M(5),
            S(Mar, [C(8, 0), P(1), P(2)]),
            D(127),
            Tc(C(20, 5)),
        ],
    ),
];

/// Ending A (§4.5, `0000:2DDE`): death once, the pilot dossier.
const ENDING_A: &[Step] = &[M(12), LoopOff, Dossier, LoopOn];
/// Ending B (`0000:2E1B`): death once, `intro2` with 20.6 / 20.7.
const ENDING_B: &[Step] = &[M(12), LoopOff, S(In2, [C(20, 6), C(20, 7), B]), LoopOn];
/// Ending C (`0000:2E5E`): death once, `intro2` with 20.8.
const ENDING_C: &[Step] = &[M(12), LoopOff, S(In2, [C(20, 8), B, B]), LoopOn];

/// What a block hands back to the menu (§4.6; E3: e3-program-flow §4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockResult {
    /// `g_MissionAdvance`.
    pub advance: bool,
    /// `g_GameOver` (endings A, B, C).
    pub game_over: bool,
    /// Kills of the last flight (`g_MissionKills`).
    pub mission_kills: i16,
    /// Blocks 8, 17: the outcome retry was declined, mission −= 1 before the return, so the
    /// menu's mission += 1 offers the same block again (kills are still added, OB-027).
    pub replay: bool,
    /// E3 block 6: `g_MissionNumber` := 7 before the return (e3-program-flow §4.4.1).
    pub set_mission: Option<u8>,
    /// E3: the skip flag at the return (`Main_CampaignLoop` no longer clears it, OB-050).
    pub skip: bool,
}

/// Which retry prompt is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    /// After a death.
    Died,
    /// Blocks 17, 18: the mission was lost.
    Lost,
    /// After the debrief, outcome 0 (blocks 8, 16, 17).
    Outcome,
    /// Block 3, after D247.
    Block3,
}

/// Where the block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pos {
    Pre(usize),
    Fly,
    Prompt(Prompt),
    Debrief,
    Post(usize),
    Ending(&'static [Step], usize),
    Return,
}

/// The screen a block step is showing (shared by the E2 and E3 sequencers).
pub(crate) enum Child {
    Story(Box<StoryScreen>),
    Dialogue(Box<DialogueScene>),
    Flight(Box<dyn FlightScreen>),
}

impl Child {
    /// Ticks per second of the screen (BUG-5): a flight's own rate, else the retrace rate.
    pub(crate) fn tick_hz(child: Option<&Self>) -> f64 {
        match child {
            Some(Child::Story(s)) => s.tick_hz(),
            Some(Child::Dialogue(d)) => d.tick_hz(),
            Some(Child::Flight(f)) => f.tick_hz(),
            None => RETRACE_HZ,
        }
    }

    /// Runs the screen for one frame; true when it is over.
    pub(crate) fn step(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> bool {
        let waits = ctx.settings.accessibility.text_waits_for_key;
        match self {
            Child::Story(s) => {
                s.set_text_waits(waits);
                s.step(input);
                s.finished()
            }
            Child::Dialogue(d) => {
                d.set_text_waits(waits);
                d.step(input);
                d.finished()
            }
            Child::Flight(f) => matches!(f.tick(ctx, input), Transition::Pop),
        }
    }

    /// What a story screen or a dialogue scene leaves in the front-end state, `g_Palette`
    /// first (screens rev 3 §2.1: the next title card fades it); a flight does not touch it.
    pub(crate) fn apply_front_end(&self, fe: &mut FrontEnd) {
        match self {
            Child::Story(s) => s.apply_front_end(fe),
            Child::Dialogue(d) => d.apply_front_end(fe),
            Child::Flight(_) => {}
        }
    }

    /// True if Esc ended a story screen or a dialogue scene.
    pub(crate) fn skipped(&self) -> bool {
        match self {
            Child::Story(s) => s.skipped(),
            Child::Dialogue(d) => d.skipped(),
            Child::Flight(_) => false,
        }
    }

    /// The last name the finished screen assigned to `g_SceneFile` (weapons rev 5 §3.3.2 item
    /// 8, §3.3.3): `td.pal` for a dialogue scene, `story_file` for a story screen (`td.lt2` for
    /// a scene card; `td.pal` for the retry prompt, debrief and dossier; none for a title card).
    pub(crate) fn scene_file<'a>(&self, story_file: Option<&'a str>) -> Option<&'a str> {
        match self {
            Child::Story(_) => story_file,
            Child::Dialogue(_) => Some(SCENE_FILE_PALETTE),
            Child::Flight(_) => None,
        }
    }

    /// Draws the screen; `None` draws the black between screens.
    pub(crate) fn draw(child: Option<&Self>, fb: &mut Framebuffer) {
        match child {
            Some(Child::Story(s)) => s.draw(fb),
            Some(Child::Dialogue(d)) => d.draw(fb),
            Some(Child::Flight(f)) => f.draw(fb),
            None => {
                fb.clear(0);
                fb.palette = Default::default();
            }
        }
    }
}

/// One Episode 2 mission block in progress.
pub struct CampaignE2 {
    mission: u8,
    kills_to_date: i16,
    block: Block,
    pos: Pos,
    child: Option<Child>,
    /// The step the child belongs to.
    child_step: Option<Step>,
    last: FlightResult,
    result: BlockResult,
}

impl fmt::Debug for CampaignE2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CampaignE2")
            .field("mission", &self.mission)
            .field("pos", &self.pos)
            .finish_non_exhaustive()
    }
}

/// The text of a caption ("" if unavailable), in the program's episode.
pub fn caption(prog: &Program, cap: Cap) -> String {
    let (id, k) = cap.resource_in(prog.episode);
    line(&prog.texts(&id), k)
}

/// The screen of a story step: title cards and scene cards.
pub fn story(prog: &Program, step: Step) -> Result<StoryScreen, StoryError> {
    match step {
        T(n) => {
            let id = format!("e2.mission.{n:02}.title");
            let text = prog.res.text(&id).unwrap_or_default().to_owned();
            Ok(scenes_e2::title_card(
                &prog.assets,
                &text,
                &prog.scene_palette(),
            ))
        }
        Tc(cap) => Ok(scenes_e2::title_card(
            &prog.assets,
            &caption(prog, cap),
            &prog.scene_palette(),
        )),
        S(bg, caps) => {
            let t = caps.map(|c| caption(prog, c));
            scenes_e2::scene_card(&prog.data, &prog.assets, bg, [&t[0], &t[1], &t[2]])
        }
        other => Err(StoryError::Text(format!("{other:?} is not a story screen"))),
    }
}

impl CampaignE2 {
    /// Ticks per second of the block's current screen (BUG-5).
    pub fn tick_hz(&self) -> f64 {
        Child::tick_hz(self.child.as_ref())
    }

    /// Block `mission` (1..=20) with `kills_to_date` before it.
    ///
    /// # Panics
    /// If `mission` is not 1..=20.
    pub fn new(mission: u8, kills_to_date: i16) -> Self {
        assert!((1..=20).contains(&mission), "mission {mission}");
        Self {
            mission,
            kills_to_date,
            block: E2_BLOCKS[usize::from(mission) - 1],
            pos: Pos::Pre(0),
            child: None,
            child_step: None,
            last: FlightResult::default(),
            result: BlockResult {
                advance: true,
                ..BlockResult::default()
            },
        }
    }

    /// True while a retry prompt is shown.
    pub fn in_retry_prompt(&self) -> bool {
        matches!(self.pos, Pos::Prompt(_))
    }

    /// True while a flight runs.
    pub fn in_flight(&self) -> bool {
        matches!(self.child, Some(Child::Flight(_)))
    }

    /// True while the debrief is shown.
    pub fn in_debrief(&self) -> bool {
        self.pos == Pos::Debrief && self.child.is_some()
    }

    /// The story step on screen, if any.
    pub fn current_step(&self) -> Option<Step> {
        self.child.as_ref().and(self.child_step)
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
                if !child.step(ctx, inp) {
                    return None;
                }
                let child = self.child.take();
                if let Some(c) = &child {
                    c.apply_front_end(&mut prog.front_end);
                }
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
        Child::draw(self.child.as_ref(), fb);
    }

    fn steps(&self) -> &'static [Step] {
        match self.pos {
            Pos::Pre(_) => self.block.pre,
            Pos::Post(_) => self.block.post,
            Pos::Ending(steps, _) => steps,
            _ => &[],
        }
    }

    /// A child finished: read its skip flag or result and move on.
    fn finish_child(
        &mut self,
        prog: &mut Program,
        ctx: &mut FlowContext<'_>,
        child: Option<Child>,
    ) {
        let skipped = child.as_ref().is_some_and(Child::skipped);
        let step = self.child_step.take();
        // The name the screen left in g_SceneFile for the flights (weapons rev 5 §3.3.2 item 8;
        // its g_Palette writes are applied by `step`).
        let story_file = match (self.pos, step) {
            (Pos::Prompt(_) | Pos::Debrief, _) | (_, Some(Dossier)) => Some(SCENE_FILE_PALETTE),
            (_, Some(S(..))) => Some(SCENE_FILE_CARDS),
            _ => None,
        };
        if let Some(name) = child.as_ref().and_then(|c| c.scene_file(story_file)) {
            prog.front_end.assign_scene_file(name);
        }
        self.pos = match (self.pos, child) {
            (Pos::Fly, Some(Child::Flight(f))) => {
                prog.cues.stop(ctx.audio);
                self.last = f.result();
                self.result.mission_kills = self.last.kills;
                self.after_flight(0)
            }
            (Pos::Prompt(p), Some(Child::Story(s))) => {
                if s.last_key() != Some(Key::ESCAPE) {
                    Pos::Fly
                } else {
                    match p {
                        Prompt::Died => self.after_flight(1),
                        Prompt::Lost => self.after_flight(2),
                        Prompt::Block3 => self.ending(ENDING_A),
                        Prompt::Outcome if self.block.outcome == OutcomeRule::EndingB => {
                            self.ending(ENDING_B)
                        }
                        Prompt::Outcome => {
                            self.result.replay = true;
                            Pos::Return
                        }
                    }
                }
            }
            (Pos::Debrief, _) => {
                if self.block.outcome != OutcomeRule::None && !self.last.outcome {
                    Pos::Prompt(Prompt::Outcome)
                } else {
                    Pos::Post(0)
                }
            }
            (Pos::Post(i), _) if self.block.post.get(i) == Some(&Outcome3) => {
                if step == Some(D(247)) {
                    // No skip test after D247; its retry prompt follows.
                    Pos::Prompt(Prompt::Block3)
                } else if skipped {
                    Pos::Return
                } else {
                    Pos::Post(i + 1)
                }
            }
            (Pos::Pre(i), _) => {
                let tested = self.block.pre.get(i + 1) != Some(&Nt);
                if skipped && tested {
                    Pos::Fly
                } else {
                    Pos::Pre(i + 1)
                }
            }
            (Pos::Post(i), _) => {
                let tested = self.block.post.get(i + 1) != Some(&Nt);
                if skipped && tested {
                    Pos::Return
                } else {
                    Pos::Post(i + 1)
                }
            }
            (Pos::Ending(steps, i), _) => Pos::Ending(steps, i + 1),
            (pos, _) => pos,
        };
    }

    /// The flight handling of §4.3 from check `from` on (0 = the death prompt, 1 = the lost
    /// prompt, 2 = the endings and the abort).
    fn after_flight(&mut self, from: u8) -> Pos {
        let r = self.last;
        let lost = self.block.lost_rules && r.lost;
        if from < 1 && r.died {
            return Pos::Prompt(Prompt::Died);
        }
        if from < 2 && lost {
            return Pos::Prompt(Prompt::Lost);
        }
        if lost {
            return self.ending(ENDING_C);
        }
        if r.died {
            return self.ending(ENDING_A);
        }
        if r.aborted {
            self.result.advance = false;
            return Pos::Return;
        }
        Pos::Debrief
    }

    fn ending(&mut self, steps: &'static [Step]) -> Pos {
        self.result.game_over = true;
        self.result.advance = false;
        Pos::Ending(steps, 0)
    }

    /// Starts whatever comes next at the current position.
    fn advance(&mut self, prog: &mut Program, ctx: &mut FlowContext<'_>) {
        let step = match self.pos {
            Pos::Pre(i) | Pos::Post(i) | Pos::Ending(_, i) => match self.steps().get(i) {
                Some(s) => *s,
                None => {
                    self.pos = match self.pos {
                        Pos::Pre(_) => Pos::Fly,
                        _ => Pos::Return,
                    };
                    return;
                }
            },
            Pos::Fly => {
                self.launch(prog, ctx);
                return;
            }
            Pos::Debrief => {
                prog.play_cue(ctx, td_audio::music_cues::LOW);
                let s = prog.debrief(
                    self.mission,
                    self.last.kills,
                    self.kills_to_date,
                    self.last.outcome,
                );
                self.child = Some(Child::Story(Box::new(s)));
                return;
            }
            Pos::Prompt(_) => {
                // `UI_RetryPrompt`: one Random(10) per prompt.
                self.child = Some(Child::Story(Box::new(prog.retry_prompt())));
                return;
            }
            Pos::Return => return,
        };
        self.child_step = Some(step);
        match step {
            M(n) => prog.play_cue(ctx, n),
            LoopOff => prog.cues.set_loop(false, ctx.audio),
            LoopOn => prog.cues.set_loop(true, ctx.audio),
            Nt => {}
            Dossier => {
                let total = self.kills_to_date.saturating_add(self.last.kills);
                self.child = Some(Child::Story(Box::new(prog.player_dossier(total))));
            }
            D(id) => self.child = Some(Child::Dialogue(Box::new(prog.dialogue(id)))),
            Outcome3 => {
                let id = if self.last.outcome { 14 } else { 247 };
                self.child_step = Some(D(id));
                self.child = Some(Child::Dialogue(Box::new(prog.dialogue(id))));
            }
            other => match story(prog, other) {
                Ok(s) => self.child = Some(Child::Story(Box::new(s))),
                Err(e) => tracing::warn!("story step {other:?} unavailable: {e}"),
            },
        }
        if self.child.is_none() {
            // Instant steps (music, loop flag, unavailable screens) move on at once.
            self.finish_child(prog, ctx, None);
        }
    }

    /// `Sim_FlyMission(block number)`: the music stops, the flight song starts; the launcher
    /// gets the front-end state the flight reads.
    fn launch(&mut self, prog: &mut Program, ctx: &mut FlowContext<'_>) {
        prog.cues.stop(ctx.audio);
        prog.play_flight_song(ctx, self.mission);
        prog.launcher.set_front_end(&prog.front_end);
        let f = prog.launcher.launch(ctx, self.mission);
        self.child = Some(Child::Flight(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_steps() -> impl Iterator<Item = &'static Step> {
        E2_BLOCKS
            .iter()
            .flat_map(|b| b.pre.iter().chain(b.post))
            .chain(ENDING_A)
            .chain(ENDING_B)
            .chain(ENDING_C)
    }

    #[test]
    fn dialogue_coverage() {
        // §4.4 notes and §6 vector 12: 124 distinct scenes, each once, never 33.
        let mut ids: Vec<u8> = all_steps()
            .filter_map(|s| match *s {
                D(n) => Some(n),
                _ => None,
            })
            .chain([14, 247])
            .collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "no scene twice");
        assert_eq!(n, 124);
        assert!(!ids.contains(&33));
    }

    #[test]
    fn music_calls() {
        // §3.1: 142 campaign music calls (the counts per song add up to it).
        let mut count = [0u32; 70];
        for s in all_steps() {
            if let M(n) = *s {
                count[usize::from(n)] += 1;
            }
        }
        // Block 3's M6 before the outcome branch and the 20 debrief cues (low).
        count[8] += 20;
        assert_eq!(count[3], 22, "hangar");
        assert_eq!(count[4], 19, "satair");
        assert_eq!(count[5], 19, "vulture");
        assert_eq!(count[6], 18, "mood");
        assert_eq!(count[10], 23, "mood2");
        assert_eq!(count[11], 5, "moodmaj");
        assert_eq!(count[12], 4, "death: 3 endings + block 20");
        assert_eq!(count[13], 12, "mood3");
        assert_eq!(count.iter().sum::<u32>(), 142);
    }

    #[test]
    fn untested_steps_follow_a_screen() {
        for b in &E2_BLOCKS {
            for part in [b.pre, b.post] {
                for w in part.windows(2) {
                    if w[1] == Nt {
                        assert!(matches!(w[0], S(..) | D(_)), "{w:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn rules_per_block() {
        let rules: Vec<(usize, OutcomeRule, bool)> = E2_BLOCKS
            .iter()
            .enumerate()
            .filter(|(_, b)| b.outcome != OutcomeRule::None || b.lost_rules)
            .map(|(i, b)| (i + 1, b.outcome, b.lost_rules))
            .collect();
        assert_eq!(
            rules,
            [
                (8, OutcomeRule::Replay, false),
                (16, OutcomeRule::EndingB, false),
                (17, OutcomeRule::Replay, true),
                (18, OutcomeRule::None, true),
            ]
        );
        assert!(E2_BLOCKS[2].post.contains(&Outcome3));
    }
}
