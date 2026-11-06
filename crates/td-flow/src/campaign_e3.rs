//! The Episode 3 campaign sequencer (`td3.exe` `Main_CampaignLoop` `0000:0BC5`): one mission
//! block per call (18 blocks, 20 flights), with its retries, debriefs, outcome rules and
//! ending A.
//!
//! Spec: `docs/spec/flow/e3-program-flow.md` rev 1 §3.4 (flight hand-off), §3.5 (skip flag),
//! §4.3 (one block), §4.4 (the 18 blocks, transcribed into [`E3_BLOCKS`]), §4.4.1 (block 6:
//! F6 and F7), §4.4.2 (block 11: F11 and F21), §4.5 (ending A, the end card), §4.6
//! (bookkeeping). Scene cards per [`crate::scenes_e3`]; the shared screens (dialogue, retry
//! prompt, debrief, pilot dossier) come from [`Program`] with `episode = E3`. Fidelity: T2 for
//! the order of screens, T1 for the state handed back to the menu and the retry prompt's
//! `Random(10)`.
//!
//! A block is one list of steps that includes its flights ([`Step::F`]) and the control steps
//! of §4.3–§4.4. Skip tests (§4.4): every screen step is followed by a skip test unless the
//! step after it is [`Step::Nt`]; a set flag jumps to the next flight of the block, or ends the
//! block when no flight follows (the mission still counts). Music, debriefs and retry prompts
//! carry no test. The skip flag is the program's (`g_SkipRequested`): the menu hands it in and
//! gets it back, because `Main_CampaignLoop` no longer clears it and block 1 tests it before
//! its first step (OB-050).

use std::fmt;

use td_input::{Key, TickInput};
use td_render::Framebuffer;

use crate::campaign::FlightResult;
use crate::campaign_e2::{BlockResult, Cap, Child, caption};
use crate::program::Program;
use crate::program_e2::BlockStart;
use crate::scenes_e3::{self, Bg};
use crate::screen::FlowContext;
use crate::story::{StoryError, StoryScreen};

/// What a declined retry prompt does (§4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decline {
    /// Block 8: continue as if the outcome were 1.
    Continue,
    /// Block 10: advance := 0, aborted := 1, return.
    Abort,
}

/// One step of an E3 block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `Mn`: music cue n (`td_audio::CueTable::E3`).
    M(u8),
    /// `Dn`: dialogue scene n.
    D(u8),
    /// `Tnn`: the title card of mission nn.
    T(u8),
    /// `T(x)`: a title-style card with a caption.
    Tc(Cap),
    /// `T(end)`: the end card (`e3.ending`).
    TEnd,
    /// `bg[a/b/c]`: a scene card.
    S(Bg, [Cap; 3]),
    /// `'`: the step before has no skip test.
    Nt,
    /// Block 1: the skip test before the first step (OB-050).
    Test,
    /// The retry target: an accepted retry prompt flies again from here (the flight call − 2).
    Mark,
    /// `Sim_FlyMission(n)`, then: died → retry prompt (accept → [`Step::Mark`], decline →
    /// ending A); aborted → return with advance 0.
    F(u8),
    /// Flight 21 (block 11): the debrief first, then died → retry prompt (accept →
    /// [`Step::Mark`], i.e. F11), ending A; aborted → return (§4.4.2).
    F21,
    /// `B`: M8, then the debrief of the last flight.
    B,
    /// Blocks 2, 3, 4, 5, 9, 12 after `B`: outcome 0 → retry prompt (accept → fly again;
    /// decline → mission −= 1, return with advance 1).
    Replay,
    /// Skip the next n steps unless the outcome is 0.
    IfFail(u8),
    /// Skip the next n steps unless the outcome is 1.
    IfOk(u8),
    /// A retry prompt: accept → fly again from [`Step::Mark`]; decline as given.
    R(Decline),
    /// Block 6: `g_MissionNumber` := 7 (`0000:171F`).
    Mission7,
    /// Ending A: `WORX_SetMusicLoop(0)` / `(1)` around the dossier.
    LoopOff,
    LoopOn,
    /// Ending A: the pilot dossier (kills to date + mission kills).
    Dossier,
}

use Bg::*;
use Cap::{C, P};
use Step::*;

/// `_`: `e3.story.preamble[2]`, a single space (§2).
pub const BLANK: Cap = P(2);
const B_: Cap = BLANK;

/// One block's steps.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub number: u8,
    pub steps: &'static [Step],
}

const fn block(number: u8, steps: &'static [Step]) -> Block {
    Block { number, steps }
}

/// The 18 Episode 3 blocks, transcribed from e3-program-flow rev 1 §4.4.
pub const E3_BLOCKS: [Block; 18] = [
    block(
        1,
        &[
            Test,
            M(6),
            S(Sh0, [P(0), P(1), B_]),
            D(1),
            Tc(P(3)),
            D(2),
            S(Sh0, [P(4), P(5), B_]),
            D(3),
            S(Sh0, [P(6), P(7), B_]),
            D(4),
            M(5),
            S(Mar, [P(8), P(9), P(10)]),
            D(5),
            T(1),
            M(10),
            S(Mn1, [P(10), C(1, 0), B_]),
            D(6),
            M(3),
            S(Mn1, [C(1, 1), C(1, 2), B_]),
            D(7),
            Mark,
            F(1),
            B,
            M(6),
            S(Mn1, [C(1, 1), C(1, 3), B_]),
            D(8),
            M(4),
            S(Cmd, [C(1, 4), C(1, 5), B_]),
            D(201),
        ],
    ),
    block(
        2,
        &[
            M(6),
            S(Cmd, [C(1, 4), C(1, 6), B_]),
            D(202),
            T(2),
            D(11),
            M(3),
            S(Mn1, [C(1, 1), C(1, 2), B_]),
            D(12),
            Mark,
            F(2),
            B,
            Replay,
            M(3),
            S(Mn1, [C(1, 1), C(2, 0), B_]),
            D(203),
            M(6),
            S(Lab, [C(2, 1), C(2, 2), B_]),
            D(204),
            M(5),
            S(Mr2, [C(2, 3), C(2, 4), P(10)]),
            D(14),
            M(10),
            S(Sh0, [P(7), P(6), B_]),
            D(15),
            D(16),
        ],
    ),
    block(
        3,
        &[
            M(13),
            S(Cnv, [C(2, 5), C(2, 6), B_]),
            D(17),
            M(4),
            S(Cmd, [C(1, 4), P(10), B_]),
            D(18),
            M(10),
            S(Cmd, [C(2, 7), C(2, 8), B_]),
            D(19),
            Mark,
            F(3),
            B,
            Replay,
            M(3),
            S(Mn1, [C(1, 1), C(2, 0), B_]),
            D(20),
            M(13),
            S(Cmd, [C(2, 7), P(10), B_]),
            D(21),
            D(22),
            M(6),
            S(Lab, [C(2, 1), C(2, 9), B_]),
            D(23),
        ],
    ),
    block(
        4,
        &[
            T(4),
            M(10),
            S(Lab, [C(2, 1), P(10), B_]),
            D(24),
            M(6),
            S(Cnv, [C(4, 0), P(10), B_]),
            D(124),
            Tc(C(4, 1)),
            D(25),
            Mark,
            F(4),
            B,
            Replay,
            M(3),
            S(Mn1, [C(1, 1), C(2, 0), B_]),
            D(26),
            M(5),
            S(Mr2, [P(8), C(4, 2), P(10)]),
            D(27),
        ],
    ),
    block(
        5,
        &[
            M(6),
            S(Cnv, [C(4, 3), P(10), B_]),
            D(28),
            T(5),
            M(4),
            S(Cmd, [C(2, 7), C(5, 0), B_]),
            D(29),
            M(13),
            S(Cnv, [C(4, 3), P(10), B_]),
            D(30),
            M(4),
            S(Cmd, [C(2, 7), C(2, 8), B_]),
            D(31),
            M(6),
            S(Cmd, [C(2, 7), C(5, 1), B_]),
            D(32),
            M(3),
            S(Mn1, [C(1, 1), C(2, 8), B_]),
            D(33),
            Mark,
            F(5),
            B,
            Replay,
            M(5),
            S(Mr2, [P(8), C(4, 2), P(10)]),
            D(34),
        ],
    ),
    block(
        6,
        &[
            M(5),
            S(Mr3, [C(5, 2), B_, B_]),
            D(35),
            T(6),
            M(4),
            S(Cmd, [C(2, 7), C(5, 1), B_]),
            D(234),
            M(6),
            Tc(P(10)),
            D(36),
            M(3),
            S(Mn1, [C(1, 1), C(1, 2), B_]),
            D(37),
            Mark,
            F(6),
            B,
            M(5),
            S(Mr3, [C(6, 0), B_, B_]),
            D(38),
            S(Kol, [C(6, 1), C(6, 2), B_]),
            D(39),
            T(7),
            D(40),
            M(13),
            Tc(P(7)),
            D(41),
            Mark,
            F(7),
            B,
            Mission7,
            M(10),
            S(Mn2, [C(7, 0), C(1, 3), B_]),
            D(42),
            M(6),
            S(Lab, [C(2, 1), C(1, 5), B_]),
            D(43),
            M(5),
            S(Kol, [C(6, 1), P(10), B_]),
            D(44),
            S(Sel, [C(7, 1), B_, B_]),
        ],
    ),
    block(
        8,
        &[
            M(5),
            S(Mr3, [C(7, 2), B_, B_]),
            D(46),
            T(8),
            S(Kol, [C(6, 1), P(7), B_]),
            S(Sel, [C(8, 0), B_, B_]),
            D(48),
            M(3),
            S(Mn2, [C(1, 1), C(1, 2), B_]),
            D(47),
            Mark,
            F(8),
            B,
            IfFail(6),
            M(3),
            S(Mn2, [C(1, 1), C(1, 3), B_]),
            Nt,
            D(49),
            Nt,
            R(Decline::Continue),
            M(10),
            S(Cmd, [C(7, 0), C(8, 1), P(10)]),
            D(50),
            M(5),
            S(Kol, [C(6, 1), C(8, 2), B_]),
            D(51),
        ],
    ),
    block(
        9,
        &[
            T(9),
            M(5),
            S(Kol, [C(6, 1), P(10), B_]),
            D(53),
            M(3),
            S(Mn2, [C(1, 1), C(1, 6), B_]),
            D(54),
            Mark,
            F(9),
            B,
            Replay,
            M(3),
            S(Mn2, [C(1, 1), C(1, 3), B_]),
            D(55),
            M(10),
            S(Kol, [C(6, 1), P(10), B_]),
            D(56),
        ],
    ),
    block(
        10,
        &[
            T(10),
            M(10),
            S(Lab, [C(2, 1), C(1, 6), B_]),
            D(57),
            M(3),
            S(Mn2, [C(1, 1), C(1, 2), B_]),
            D(58),
            Mark,
            F(10),
            B,
            M(3),
            S(Mn2, [C(1, 1), C(1, 3), B_]),
            Nt,
            IfOk(1),
            D(60),
            IfFail(3),
            D(59),
            Nt,
            R(Decline::Abort),
        ],
    ),
    block(
        11,
        &[
            T(11),
            M(5),
            S(Kol, [C(6, 1), P(10), B_]),
            D(61),
            M(4),
            S(Cmd, [C(8, 1), C(7, 0), B_]),
            D(62),
            M(3),
            S(Mn2, [C(1, 1), C(2, 8), B_]),
            D(63),
            Mark,
            F(11),
            M(13),
            D(64),
            Nt,
            F21,
            M(3),
            S(Mn2, [C(1, 1), C(1, 3), B_]),
            D(65),
        ],
    ),
    block(
        12,
        &[
            M(5),
            S(Kol, [C(6, 1), C(1, 6), B_]),
            D(66),
            T(12),
            M(3),
            S(Mn2, [C(1, 1), C(2, 8), B_]),
            D(67),
            Mark,
            F(12),
            B,
            Replay,
            M(3),
            S(Mn2, [C(1, 1), C(1, 3), B_]),
            D(68),
            M(10),
            S(Cmd, [C(7, 0), C(8, 1), C(1, 2)]),
            D(168),
        ],
    ),
    block(
        13,
        &[
            T(13),
            M(6),
            S(Lab, [C(2, 1), C(1, 6), B_]),
            D(69),
            M(3),
            S(Mn2, [C(1, 1), C(1, 2), B_]),
            D(70),
            Mark,
            F(13),
            B,
            M(13),
            S(Ion, [C(13, 0), C(1, 3), B_]),
            D(71),
        ],
    ),
    block(
        14,
        &[
            T(14),
            M(10),
            S(Kol, [C(6, 1), P(10), B_]),
            D(72),
            Tc(C(1, 5)),
            D(73),
            M(13),
            S(Vhg, [C(14, 0), C(2, 2), B_]),
            D(74),
            Mark,
            F(14),
            B,
            M(3),
            S(Mn2, [C(1, 1), C(1, 3), B_]),
            D(75),
        ],
    ),
    block(
        15,
        &[
            M(5),
            S(Kol, [C(6, 1), B_, B_]),
            D(76),
            T(15),
            M(4),
            S(Cmd, [C(7, 0), C(8, 1), B_]),
            D(77),
            M(10),
            S(Lab, [C(2, 1), C(15, 0), B_]),
            D(78),
            M(4),
            S(Cmd, [C(7, 0), C(8, 1), P(10)]),
            D(79),
            M(6),
            S(Vid, [C(15, 1), P(10), B_]),
            D(80),
            M(3),
            S(Mn2, [C(1, 1), C(2, 8), B_]),
            D(81),
            Mark,
            F(15),
            B,
            M(3),
            S(Mn2, [C(1, 1), C(1, 3), B_]),
            D(82),
        ],
    ),
    block(
        16,
        &[
            T(16),
            M(5),
            S(Kol, [C(6, 1), B_, B_]),
            D(83),
            M(10),
            S(Vid, [C(15, 1), P(10), B_]),
            D(84),
            M(3),
            S(Mn2, [C(1, 1), C(1, 5), B_]),
            D(85),
            Mark,
            F(16),
            B,
            M(6),
            S(Lab, [C(2, 1), C(1, 3), B_]),
            D(86),
        ],
    ),
    block(
        17,
        &[
            T(17),
            M(5),
            S(Kol, [C(6, 1), B_, B_]),
            D(87),
            S(Lab, [C(2, 1), C(17, 0), B_]),
            D(88),
            M(5),
            S(Vlb, [C(17, 1), P(10), B_]),
            D(90),
            M(10),
            S(Vlb, [C(17, 1), C(17, 2), B_]),
            D(91),
            Mark,
            F(17),
            B,
            M(13),
            S(Mn2, [C(17, 3), C(2, 0), B_]),
            D(92),
            M(5),
            S(Kol, [C(6, 1), P(10), B_]),
            D(93),
        ],
    ),
    block(
        18,
        &[
            T(18),
            M(10),
            S(Vid, [C(15, 1), B_, B_]),
            D(94),
            M(5),
            S(Kol, [C(6, 1), P(10), B_]),
            D(194),
            M(13),
            S(Vid, [C(15, 1), B_, B_]),
            D(95),
            Mark,
            F(18),
            B,
            M(5),
            S(Mr3, [C(18, 0), C(18, 1), C(18, 2)]),
            D(96),
            S(Mr3, [C(18, 3), C(2, 2), B_]),
            D(97),
            S(Mr3, [C(18, 4), C(18, 5), B_]),
            D(197),
            S(Mr3, [C(18, 6), C(18, 7), B_]),
            D(98),
            S(Mr3, [C(18, 8), B_, B_]),
            D(99),
            S(In2, [C(18, 9), B_, B_]),
            M(11),
            S(Mr2, [C(18, 4), C(18, 10), C(18, 11)]),
            D(100),
        ],
    ),
    block(
        19,
        &[
            M(10),
            S(Kol, [C(6, 1), B_, B_]),
            D(101),
            M(13),
            S(Sh0, [C(18, 12), B_, B_]),
            S(In2, [C(18, 13), B_, B_]),
            D(102),
            T(19),
            M(6),
            S(Kol, [C(6, 1), P(10), B_]),
            D(103),
            S(Mel, [C(19, 0), B_, B_]),
            D(105),
            M(13),
            S(Vhg, [C(14, 0), C(2, 2), B_]),
            D(106),
            Mark,
            F(19),
            M(10),
            S(Mn2, [C(19, 1), C(19, 2), B_]),
            D(107),
            M(11),
            S(Kol, [C(6, 1), B_, B_]),
            D(108),
            S(Kol, [P(10), B_, B_]),
            D(109),
            TEnd,
        ],
    ),
];

/// Ending A (§4.5, `0000:298A`): death once, the pilot dossier.
pub const ENDING_A: &[Step] = &[M(12), LoopOff, Dossier, LoopOn];

/// The block of mission `mission`, if it has one (none for 7, 20 and above, OB-051).
pub fn block_of(mission: u8) -> Option<&'static Block> {
    E3_BLOCKS.iter().find(|b| b.number == mission)
}

/// What follows a retry prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    /// After a death: decline → ending A.
    Died,
    /// [`Step::Replay`]: decline → replay.
    Replay,
    /// [`Step::R`].
    Step(Decline),
}

/// Where the block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pos {
    /// Step i of the block.
    At(usize),
    /// Flight 21's debrief, before its death and abort checks.
    Debrief21,
    Prompt(Prompt, usize),
    Ending(usize),
    Return,
}

/// One Episode 3 mission block in progress.
pub struct CampaignE3 {
    mission: u8,
    kills_to_date: i16,
    steps: &'static [Step],
    pos: Pos,
    child: Option<Child>,
    /// The step the child belongs to.
    child_step: Option<Step>,
    last: FlightResult,
    /// `g_SkipRequested`.
    skip: bool,
    result: BlockResult,
}

impl fmt::Debug for CampaignE3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CampaignE3")
            .field("mission", &self.mission)
            .field("pos", &self.pos)
            .finish_non_exhaustive()
    }
}

/// The screen of a story step: title cards, the end card and scene cards.
pub fn story(prog: &Program, step: Step) -> Result<StoryScreen, StoryError> {
    let text = |id: &str| prog.res.text(id).unwrap_or_default().to_owned();
    match step {
        T(n) => Ok(scenes_e3::title_card(
            &prog.assets,
            &text(&format!("e3.mission.{n:02}.title")),
            &prog.scene_palette,
        )),
        Tc(cap) => Ok(scenes_e3::title_card(
            &prog.assets,
            &caption(prog, cap),
            &prog.scene_palette,
        )),
        TEnd => Ok(scenes_e3::title_card(
            &prog.assets,
            &text("e3.ending"),
            &prog.scene_palette,
        )),
        S(bg, caps) => {
            let t = caps.map(|c| caption(prog, c));
            scenes_e3::scene_card(&prog.data, &prog.assets, bg, [&t[0], &t[1], &t[2]])
        }
        other => Err(StoryError::Text(format!("{other:?} is not a story screen"))),
    }
}

impl CampaignE3 {
    /// Ticks per second of the block's current screen (BUG-5).
    pub fn tick_hz(&self) -> f64 {
        Child::tick_hz(self.child.as_ref())
    }

    /// Block `start.mission` (any mission; 7, 20 and above run no block).
    pub fn new(start: BlockStart) -> Self {
        let (steps, pos) = match block_of(start.mission) {
            Some(b) => (b.steps, Pos::At(0)),
            None => (&[][..], Pos::Return),
        };
        Self {
            mission: start.mission,
            kills_to_date: start.kills_to_date,
            steps,
            pos,
            child: None,
            child_step: None,
            last: FlightResult {
                kills: start.mission_kills,
                ..FlightResult::default()
            },
            skip: start.skip,
            result: BlockResult {
                advance: true,
                mission_kills: start.mission_kills,
                skip: start.skip,
                ..BlockResult::default()
            },
        }
    }

    /// True while a retry prompt is shown.
    pub fn in_retry_prompt(&self) -> bool {
        matches!(self.pos, Pos::Prompt(..)) && self.child.is_some()
    }

    /// True while a flight runs.
    pub fn in_flight(&self) -> bool {
        matches!(self.child, Some(Child::Flight(_)))
    }

    /// True while a debrief is shown.
    pub fn in_debrief(&self) -> bool {
        self.child.is_some()
            && (self.pos == Pos::Debrief21 || matches!(self.child_step, Some(Step::B)))
    }

    /// The story step on screen, if any.
    pub fn current_step(&self) -> Option<Step> {
        self.child.as_ref().and(self.child_step)
    }

    /// The skip flag.
    pub fn skip_flag(&self) -> bool {
        self.skip
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
                if let Some(p) = child.as_ref().and_then(Child::palette_left) {
                    prog.leave_palette(p);
                }
                self.finish_child(prog, ctx, child);
                continue;
            }
            if self.pos == Pos::Return {
                self.result.skip = self.skip;
                return Some(self.result);
            }
            self.advance(prog, ctx);
        }
    }

    /// Draws the current child (black between screens).
    pub fn draw(&self, fb: &mut Framebuffer) {
        Child::draw(self.child.as_ref(), fb);
    }

    /// The index of the next flight from step `i` on (a skip target), if any.
    fn next_flight(&self, i: usize) -> Option<usize> {
        (i..self.steps.len()).find(|&j| matches!(self.steps[j], Mark | F(_) | F21))
    }

    /// The retry target before step `i`.
    fn mark_before(&self, i: usize) -> usize {
        (0..i).rev().find(|&j| self.steps[j] == Mark).unwrap_or(0)
    }

    /// After a set skip flag at step `i`: the next flight, or the end of the block.
    fn skip_from(&self, i: usize) -> Pos {
        self.next_flight(i).map_or(Pos::Return, Pos::At)
    }

    /// A child finished: read the skip flag or the flight result and move on.
    fn finish_child(
        &mut self,
        prog: &mut Program,
        ctx: &mut FlowContext<'_>,
        child: Option<Child>,
    ) {
        let step = self.child_step.take();
        match (&child, step) {
            // The retry prompt clears the flag and never sets it (§3.5).
            (Some(Child::Story(_)), _) if matches!(self.pos, Pos::Prompt(..)) => {
                self.skip = false;
            }
            // Every other screen clears it on entry and sets it on Esc.
            (Some(c @ (Child::Story(_) | Child::Dialogue(_))), _) => self.skip = c.skipped(),
            _ => {}
        }
        self.pos = match (self.pos, child) {
            (Pos::At(i), Some(Child::Flight(f))) => {
                prog.cues.stop(ctx.audio);
                self.last = f.result();
                self.result.mission_kills = self.last.kills;
                if self.steps[i] == F21 {
                    Pos::Debrief21
                } else {
                    self.after_flight(i)
                }
            }
            (Pos::Debrief21, _) => self.after_flight(self.position_of_f21()),
            (Pos::Prompt(p, i), Some(Child::Story(s))) => {
                if s.last_key() != Some(Key::ESCAPE) {
                    Pos::At(self.mark_before(i))
                } else {
                    match p {
                        Prompt::Died => self.ending(),
                        Prompt::Replay => {
                            self.result.replay = true;
                            Pos::Return
                        }
                        Prompt::Step(Decline::Continue) => Pos::At(i + 1),
                        Prompt::Step(Decline::Abort) => {
                            self.result.advance = false;
                            Pos::Return
                        }
                    }
                }
            }
            (Pos::At(i), _) => {
                let tested = !matches!(step, Some(B))
                    && self.steps.get(i + 1) != Some(&Nt)
                    && matches!(step, Some(D(_) | T(_) | Tc(_) | TEnd | S(..)));
                if tested && self.skip {
                    self.skip_from(i + 1)
                } else {
                    Pos::At(i + 1)
                }
            }
            (Pos::Ending(i), _) => Pos::Ending(i + 1),
            (pos, _) => pos,
        };
    }

    fn position_of_f21(&self) -> usize {
        self.steps.iter().position(|&s| s == F21).unwrap_or(0)
    }

    /// The flight handling of §4.3 after the flight at step `i`: died → prompt; aborted →
    /// return with advance 0; otherwise the next step.
    fn after_flight(&mut self, i: usize) -> Pos {
        if self.last.died {
            return Pos::Prompt(Prompt::Died, i);
        }
        if self.last.aborted {
            self.result.advance = false;
            return Pos::Return;
        }
        Pos::At(i + 1)
    }

    fn ending(&mut self) -> Pos {
        self.result.game_over = true;
        self.result.advance = false;
        Pos::Ending(0)
    }

    fn debrief(&mut self, prog: &mut Program, ctx: &mut FlowContext<'_>) {
        prog.play_cue(ctx, td_audio::music_cues::LOW);
        let s = prog.debrief(
            self.mission,
            self.last.kills,
            self.kills_to_date,
            self.last.outcome,
        );
        self.child_step = Some(B);
        self.child = Some(Child::Story(Box::new(s)));
    }

    /// Starts whatever comes next at the current position.
    fn advance(&mut self, prog: &mut Program, ctx: &mut FlowContext<'_>) {
        let (step, i) = match self.pos {
            Pos::At(i) => match self.steps.get(i) {
                Some(&s) => (s, i),
                None => {
                    self.pos = Pos::Return;
                    return;
                }
            },
            Pos::Ending(i) => match ENDING_A.get(i) {
                Some(&s) => (s, i),
                None => {
                    self.pos = Pos::Return;
                    return;
                }
            },
            Pos::Debrief21 => {
                self.debrief(prog, ctx);
                return;
            }
            Pos::Prompt(..) => {
                // `UI_RetryPrompt`: one Random(10) per prompt.
                self.child = Some(Child::Story(Box::new(prog.retry_prompt())));
                return;
            }
            Pos::Return => return,
        };
        self.child_step = Some(step);
        let next = |pos: Pos| match pos {
            Pos::At(i) => Pos::At(i + 1),
            Pos::Ending(i) => Pos::Ending(i + 1),
            other => other,
        };
        match step {
            M(n) => prog.play_cue(ctx, n),
            LoopOff => prog.cues.set_loop(false, ctx.audio),
            LoopOn => prog.cues.set_loop(true, ctx.audio),
            Nt | Mark => {}
            Test => {
                if self.skip {
                    // OB-050: straight to the flight; the menu song plays on.
                    self.child_step = None;
                    self.pos = self.skip_from(i + 1);
                    return;
                }
            }
            Mission7 => self.result.set_mission = Some(7),
            IfFail(n) | IfOk(n) => {
                let skip_part = (step == IfFail(n)) == self.last.outcome;
                if skip_part {
                    self.child_step = None;
                    self.pos = Pos::At(i + 1 + usize::from(n));
                    return;
                }
            }
            Replay => {
                if !self.last.outcome {
                    self.child_step = None;
                    self.pos = Pos::Prompt(Prompt::Replay, i);
                    return;
                }
            }
            R(d) => {
                self.child_step = None;
                self.pos = Pos::Prompt(Prompt::Step(d), i);
                return;
            }
            F(n) => {
                self.child_step = None;
                self.launch(prog, ctx, n);
                return;
            }
            F21 => {
                self.child_step = None;
                self.launch(prog, ctx, 21);
                return;
            }
            B => {
                self.debrief(prog, ctx);
                return;
            }
            Dossier => {
                let total = self.kills_to_date.saturating_add(self.last.kills);
                let s = prog.player_dossier(total);
                self.child = Some(Child::Story(Box::new(s)));
            }
            D(id) => self.child = Some(Child::Dialogue(Box::new(prog.dialogue(id)))),
            other => match story(prog, other) {
                Ok(s) => self.child = Some(Child::Story(Box::new(s))),
                Err(e) => tracing::warn!("story step {other:?} unavailable: {e}"),
            },
        }
        if self.child.is_none() {
            // Instant steps (music, loop flag, marks, unavailable screens) move on at once.
            self.child_step = None;
            self.pos = next(self.pos);
        }
    }

    /// `Sim_FlyMission(flight)`: the music stops, the flight song starts.
    fn launch(&mut self, prog: &mut Program, ctx: &mut FlowContext<'_>, flight: u8) {
        prog.cues.stop(ctx.audio);
        prog.play_flight_song(ctx, flight);
        let f = prog.launcher.launch(ctx, flight);
        self.child = Some(Child::Flight(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_steps() -> impl Iterator<Item = &'static Step> {
        E3_BLOCKS
            .iter()
            .flat_map(|b| b.steps.iter())
            .chain(ENDING_A)
    }

    #[test]
    fn eighteen_blocks_twenty_flights() {
        let numbers: Vec<u8> = E3_BLOCKS.iter().map(|b| b.number).collect();
        let want: Vec<u8> = (1..=6).chain(8..=19).collect();
        assert_eq!(numbers, want);
        let mut flights: Vec<u8> = all_steps()
            .filter_map(|s| match *s {
                F(n) => Some(n),
                F21 => Some(21),
                _ => None,
            })
            .collect();
        flights.sort_unstable();
        let want: Vec<u8> = (1..=19).chain([21]).collect();
        assert_eq!(flights, want);
        assert!(block_of(7).is_none() && block_of(20).is_none());
    }

    #[test]
    fn dialogue_coverage() {
        // §4.4 notes, §6 vector 15: 111 distinct scenes, each once.
        let mut ids: Vec<u8> = all_steps()
            .filter_map(|s| match *s {
                D(n) => Some(n),
                _ => None,
            })
            .collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "no scene twice");
        assert_eq!(n, 111);
    }

    #[test]
    fn music_calls() {
        // §3.1: 111 campaign music calls; M8 before each of the 18 debriefs, M12 once.
        let mut count = [0u32; 70];
        for s in all_steps() {
            match *s {
                M(n) => count[usize::from(n)] += 1,
                B | F21 => count[8] += 1,
                _ => {}
            }
        }
        assert_eq!(count[3], 22, "hangar");
        assert_eq!(count[4], 8, "satair");
        assert_eq!(count[5], 19, "vulture");
        assert_eq!(count[6], 14, "mood");
        assert_eq!(count[8], 18, "low");
        assert_eq!(count[10], 16, "mood2");
        assert_eq!(count[11], 2, "moodmaj");
        assert_eq!(count[12], 1, "death");
        assert_eq!(count[13], 11, "mood3");
        assert_eq!(count.iter().sum::<u32>(), 111);
    }

    #[test]
    fn untested_steps_follow_a_screen_and_marks_precede_flights() {
        for b in &E3_BLOCKS {
            for w in b.steps.windows(2) {
                if w[1] == Nt {
                    assert!(matches!(w[0], S(..) | D(_)), "{w:?}");
                }
                if w[0] == Mark {
                    assert!(matches!(w[1], F(_)), "block {}: {w:?}", b.number);
                }
            }
            assert!(b.steps.contains(&Mark), "block {}", b.number);
        }
        // Block 1 tests the flag before its first step (OB-050); no other block does.
        assert_eq!(E3_BLOCKS[0].steps[0], Test);
        assert_eq!(all_steps().filter(|&&s| s == Test).count(), 1);
    }

    #[test]
    fn rules_per_block() {
        let with = |f: fn(&Step) -> bool| -> Vec<u8> {
            E3_BLOCKS
                .iter()
                .filter(|b| b.steps.iter().any(f))
                .map(|b| b.number)
                .collect()
        };
        assert_eq!(with(|s| *s == Replay), [2, 3, 4, 5, 9, 12]);
        assert_eq!(with(|s| *s == R(Decline::Continue)), [8]);
        assert_eq!(with(|s| *s == R(Decline::Abort)), [10]);
        assert_eq!(with(|s| *s == F21), [11]);
        assert_eq!(with(|s| *s == Mission7), [6]);
        assert_eq!(with(|s| *s == TEnd), [19]);
        // Block 19 has no debrief; block 11's only debrief is F21's.
        assert!(!block_of(19).unwrap().steps.contains(&B));
        assert!(!block_of(11).unwrap().steps.contains(&B));
    }
}
