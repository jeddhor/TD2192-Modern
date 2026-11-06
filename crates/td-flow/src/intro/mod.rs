//! The intro sequence (`file0000.exe`): the date card, three narration cards on `intro0.scr`,
//! the desert flyover with the credits, the `intro1.scr` and `intro2.scr` cards, and the exit.
//!
//! Spec: `docs/spec/flow/intro.md` (rev 3); `docs/spec/flow/launcher.md` (rev 1) §4.3, §4.10;
//! `docs/spec/render/text.md` (rev 2); `docs/spec/audio/cmf.md`, `music-cues.md`.
//! Fidelity: T0 for every drawn frame and T1 for the flyover state (C-0047, C-0081, see
//! `tests/intro_parity.rs`); T2 for the pacing (P1, intro §4.3) and the audio.
//!
//! The intro draws into a model of the unchained VGA memory ([`td_render::vga`]) with the
//! original's addresses, so page flips, the tile layout, OB-058 and OB-059 come out as in the
//! original. It is a small interpreter over a list of [`Op`]s, one 70.086 Hz frame per tick:
//!
//! - a full fade writes one level per frame; the code after it runs in the next frame (the
//!   original's last level spills over the frame boundary, intro §4.3);
//! - a colour fade writes 10 levels per frame, starting at the next retrace not yet used;
//! - a hold counts frames (132 / 366 / 334, the P1 values of intro §4.3; OB-034 fixed) and ends
//!   at once while a key is waiting;
//! - a flyover step runs once per BIOS tick (19.20 Hz, 3 or 4 frames).
//!
//! Port policies: OB-058 (the hoverskid's low-byte y) is kept unless
//! [`IntroOptions::fix_ob058`] (Enhanced); an effect counts as playing for the measured busy
//! time of intro rev 3 §3.6 ([`flyover::Effect::busy_half_frames`], U-380), not for its sample
//! length; a song played with the loop off restarts `intro2.cmf` at the first step after its
//! end (§4.6, U-370); the effect volumes of intro §3.6 are not applied (the mixer plays every
//! effect at full volume, T2); keys still waiting when the intro ends are dropped.

pub mod flyover;

use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use td_assets::{AssetError, DataDir, ExeResources};
use td_audio::{AudioCommand, Pcm8};
use td_formats::FormatError;
use td_formats::cmf::Cmf;
use td_formats::font::{Font, FontKind};
use td_formats::palette::{Palette, Rgb6};
use td_formats::screen::Screen as ScrImage;
use td_input::{Key, TickInput};
use td_render::intro::{self as draw, Hoverskid, TextStyle};
use td_render::vga::VgaMemory;
use td_render::{Framebuffer, WIDTH};
use thiserror::Error;

use crate::dialogue::KEY_BUFFER;
use crate::screen::{FlowContext, Screen, Transition};
use crate::story::is_keystroke;
use flyover::{Caption, Effect, FlyoverState, FrameClock};

/// Errors while loading the intro.
#[derive(Debug, Error)]
pub enum IntroError {
    #[error(transparent)]
    Asset(#[from] AssetError),
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error("{0} is too short")]
    Short(&'static str),
}

/// Frames of `Hold(4)` (date card), measured at P1 (intro §4.3).
pub const HOLD_DATE: u32 = 132;
/// Frames of `Hold(11)` (`intro0` cards and `intro2` lines).
pub const HOLD_11: u32 = 366;
/// Frames of `Hold(10)` (`intro1` lines).
pub const HOLD_10: u32 = 334;
/// `Crt.Delay(100)` after the flyover's fade-out, in frames (intro §3.7).
pub const DELAY_AFTER_FLYOVER: u32 = 7;
/// Levels a colour fade writes per frame at P1 (intro §4.3).
pub const COLOUR_LEVELS_PER_FRAME: u8 = 10;

/// Start addresses (intro §4.7–§4.9).
const START_DATE: u16 = 0x44C0;
const START_PLANET: u16 = 0x0000;
const START_PAGE0: u16 = 0x0B00;
const START_INTRO2: u16 = 0x4FC0;
/// Rows the clear-page primitive zeroes (intro §4.2).
const CLEAR_ROWS: usize = 232;
/// `intro0.scr` palette entries set to black after loading (intro §3.4).
const ZEROED_INKS: [u8; 4] = [52, 92, 195, 221];
/// Rows in a saved strip (intro §3.1).
const STRIP_ROWS: usize = 7;

/// When the intro plays before Episode 1 (launcher §4.10). Only the Enhanced profile reads it:
/// Classic always plays the intro, as `td1.exe` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntroPolicy {
    #[default]
    Always,
    /// Only until it has been shown once ([`IntroSettings::seen`]).
    FirstLaunch,
    Never,
}

/// The `[intro]` section of `settings.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IntroSettings {
    pub play: IntroPolicy,
    /// Set once the intro has been shown (for [`IntroPolicy::FirstLaunch`]).
    pub seen: bool,
}

impl IntroSettings {
    /// Whether starting Episode 1 plays the intro now.
    #[must_use]
    pub fn plays(&self, enhanced: bool) -> bool {
        !enhanced
            || match self.play {
                IntroPolicy::Always => true,
                IntroPolicy::FirstLaunch => !self.seen,
                IntroPolicy::Never => false,
            }
    }
}

/// Options of the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IntroOptions {
    /// Enhanced: draw the hoverskid with its full y (OB-058 fixed, intro §5).
    pub fix_ob058: bool,
}

impl IntroOptions {
    /// The options for `settings`: OB-058 is fixed under the Enhanced profile only.
    #[must_use]
    pub fn for_settings(settings: &crate::Settings) -> Self {
        Self {
            fix_ob058: settings.profile == crate::settings::Profile::Enhanced,
        }
    }

    fn hoverskid(self) -> Hoverskid {
        if self.fix_ob058 {
            Hoverskid::Clipped
        } else {
            Hoverskid::LowByte
        }
    }
}

/// What the flyover needs (also the credits screen's mode A).
#[derive(Clone)]
pub struct FlyoverAssets {
    pub font: Font,
    /// `td.shp` as loaded (20 records of 32 × 32).
    pub sprites: Vec<u8>,
    /// `intro.map` (20 × 2000, column-major).
    pub map: Vec<u8>,
    pub blk: Vec<u8>,
    pub dun: Vec<u8>,
    pub td_pal: Palette,
    /// `intro.credits` (25 strings).
    pub credits: Vec<String>,
    pub intro_song: Option<Arc<Cmf>>,
    pub intro2_song: Option<Arc<Cmf>>,
    /// The five effects in [`Effect::ALL`] order.
    pub effects: [Option<Arc<Pcm8>>; 5],
}

impl std::fmt::Debug for FlyoverAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FlyoverAssets")
    }
}

/// Reads `name`, failing if it is shorter than `len`.
fn read_at_least(data: &DataDir, name: &'static str, len: usize) -> Result<Vec<u8>, IntroError> {
    let bytes = data.read(name)?;
    if bytes.len() < len {
        return Err(IntroError::Short(name));
    }
    Ok(bytes)
}

/// A song, or `None` (with a warning) if it is missing or does not parse.
pub(crate) fn load_song(data: &DataDir, name: &str) -> Option<Arc<Cmf>> {
    match data.read(name).map(|b| Cmf::parse(&b)) {
        Ok(Ok(c)) => Some(Arc::new(c)),
        Ok(Err(e)) => {
            tracing::warn!("{name}: {e}");
            None
        }
        Err(e) => {
            tracing::warn!("{name}: {e}");
            None
        }
    }
}

fn load_effect(data: &DataDir, name: &str) -> Option<Arc<Pcm8>> {
    let bytes = data.read(name).ok()?;
    match td_audio::decode_voc(&bytes) {
        Ok(p) => Some(Arc::new(p)),
        Err(e) => {
            tracing::warn!("{name}: {e}");
            None
        }
    }
}

/// A list resource, or an empty list (with a warning).
fn texts(res: &ExeResources, id: &str) -> Vec<String> {
    match res.texts(id) {
        Some(t) => t.to_vec(),
        None => {
            tracing::warn!("text resource {id} unavailable");
            Vec::new()
        }
    }
}

impl FlyoverAssets {
    /// Loads the flyover's files (intro §3.1). The songs and effects are optional.
    pub fn load(data: &DataDir, res: &ExeResources) -> Result<Self, IntroError> {
        let mut effects: [Option<Arc<Pcm8>>; 5] = Default::default();
        for (slot, e) in effects.iter_mut().zip(Effect::ALL) {
            *slot = load_effect(data, e.file());
        }
        Ok(Self {
            font: Font::parse(&data.read("td.lt2")?, FontKind::Small)?,
            sprites: read_at_least(data, "td.shp", 20 * 1024)?,
            map: read_at_least(data, "intro.map", draw::MAP_COLUMNS * draw::MAP_ROWS)?,
            blk: read_at_least(data, "td.blk", draw::BLK_LOADED)?,
            dun: read_at_least(data, "td.dun", 3 * 1024)?,
            td_pal: Palette::parse(&data.read("td.pal")?)?,
            credits: texts(res, "intro.credits"),
            intro_song: load_song(data, "intro.cmf"),
            intro2_song: load_song(data, "intro2.cmf"),
            effects,
        })
    }

    fn effect(&self, e: Effect) -> Option<&Arc<Pcm8>> {
        let i = Effect::ALL.iter().position(|&x| x == e)?;
        self.effects[i].as_ref()
    }
}

/// Everything the whole intro needs.
#[derive(Clone)]
pub struct IntroAssets {
    pub fly: FlyoverAssets,
    pub intro0: ScrImage,
    pub intro1: ScrImage,
    pub intro2: ScrImage,
    /// `intro.narration.01` (5), `.02` (2), `.03` (2).
    pub narration: [Vec<String>; 3],
    pub intro1_song: Option<Arc<Cmf>>,
}

impl std::fmt::Debug for IntroAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IntroAssets")
    }
}

impl IntroAssets {
    /// Loads every file of intro §3.1 (`td.cfg` is not read: the port's settings decide).
    pub fn load(data: &DataDir, res: &ExeResources) -> Result<Self, IntroError> {
        let scr = |name: &str| -> Result<ScrImage, IntroError> {
            Ok(ScrImage::parse(&data.read(name)?)?)
        };
        Ok(Self {
            fly: FlyoverAssets::load(data, res)?,
            intro0: scr("intro0.scr")?,
            intro1: scr("intro1.scr")?,
            intro2: scr("intro2.scr")?,
            narration: [
                texts(res, "intro.narration.01"),
                texts(res, "intro.narration.02"),
                texts(res, "intro.narration.03"),
            ],
            intro1_song: load_song(data, "intro1.cmf"),
        })
    }
}

/// The colour a colour fade writes into its entry at level k (intro §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ink {
    /// (k, 0, 0): the date line.
    Red,
    /// (0, k, k): narration.
    Cyan,
    /// (k, k, k): `intro2`'s dark text.
    Grey,
}

impl Ink {
    fn at(self, k: u8) -> Rgb6 {
        match self {
            Ink::Red => Rgb6 { r: k, g: 0, b: 0 },
            Ink::Cyan => Rgb6 { r: 0, g: k, b: k },
            Ink::Grey => Rgb6 { r: k, g: k, b: k },
        }
    }
}

/// Which still a [`Op::LoadScr`] loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pic {
    Intro1,
    Intro2,
}

/// One step of the intro's program.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    /// Set start (`0957:0343`): waits for the next retrace, then shows from `start`.
    SetStart(u16),
    /// A full fade (65 levels, one per frame).
    FullFade {
        up: bool,
    },
    /// A colour fade of one DAC entry, 64 levels.
    ColourFade {
        entry: u8,
        ink: Ink,
        up: bool,
    },
    /// `Hold(n)`: `n` frames, or until a key is waiting.
    Hold(u32),
    /// Reads a waiting key; Esc goes to the exit.
    KeyCheck,
    /// A DAC entry (not the working palette).
    SetDac(u8, Rgb6),
    /// `Text B` of a narration line: resource list `set`, line `index`.
    Text {
        set: usize,
        index: usize,
        pen: (i32, i32),
        ink: u8,
    },
    /// `intro1.cmf` from the date card (musicState 2; the driver's default loop).
    PlayIntro1,
    /// A still: its palette to the working palette, its pixels from VGA row `row`, and the
    /// strip of image rows `strip..strip + 7`.
    LoadScr {
        pic: Pic,
        row: u16,
        strip: usize,
    },
    /// Clear page 0 or 1 (232 rows).
    ClearPage(u8),
    /// The saved strip back to VGA row `row` (x 0..319).
    RestoreStrip(u16),
    /// The flyover's setup before its fade-in (intro §4.6).
    FlySetup,
    /// The flyover's steps.
    FlySteps,
    /// `Crt.Delay`: frames.
    Delay(u32),
    /// After the flyover: `intro2.cmf` unless it is already the song.
    Intro2IfNeeded,
    StopMusic,
    /// The exit's key read: one waiting key is swallowed.
    Swallow,
    End,
}

/// Events the interpreter logs (frame, event), for tests and tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// The first level of a full fade was written.
    FullFade { up: bool },
    /// The first levels of a colour fade were written.
    ColourFade { entry: u8, up: bool },
    /// A flyover step began (s incremented and its page drawn).
    Step(u16),
    /// The program ended.
    End,
}

/// What an op asks the interpreter to do next.
enum Next {
    /// The op is done; go on in this frame.
    Continue,
    /// Wait for the next frame.
    Yield,
    /// Go to another op.
    Jump(usize),
}

/// The intro (or the credits flyover) as a screen.
pub struct Intro {
    fly_assets: Arc<FlyoverAssets>,
    cards: Option<Arc<IntroAssets>>,
    options: IntroOptions,
    script: Vec<Op>,
    /// Where Esc at a key check goes (`None`: Esc is an ordinary key).
    exit_at: Option<usize>,
    pc: usize,
    /// Progress inside the current op.
    sub: Option<u64>,
    frame: u64,
    /// The retrace of this frame is still ahead (no wait has used it).
    retrace: bool,
    vga: VgaMemory,
    dac: Palette,
    working: Palette,
    strip: Vec<u8>,
    keys: VecDeque<Key>,
    fly: FlyoverState,
    bios: FrameClock,
    song_clock: FrameClock,
    /// Song tick at which the current non-looping song ends (`None`: looping or silent).
    song_end: Option<u64>,
    music_state: u8,
    /// Half frame (2 × frame) at which the playing effect stops counting as playing: its
    /// start plus [`Effect::busy_half_frames`] (intro §3.6).
    sfx_end: u64,
    marks: Vec<(u64, Mark)>,
    done: bool,
    then: Option<Box<dyn Screen>>,
}

impl std::fmt::Debug for Intro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Intro")
            .field("frame", &self.frame)
            .field("op", &self.script.get(self.pc))
            .field("s", &self.fly.s)
            .finish_non_exhaustive()
    }
}

impl Intro {
    fn blank(fly: Arc<FlyoverAssets>, options: IntroOptions) -> Self {
        let mut vga = VgaMemory::new();
        draw::load_tiles(&mut vga, &fly.blk, &fly.dun);
        Self {
            fly_assets: fly,
            cards: None,
            options,
            script: Vec::new(),
            exit_at: None,
            pc: 0,
            sub: None,
            frame: 0,
            retrace: true,
            vga,
            dac: Palette::default(),
            working: Palette::default(),
            strip: Vec::new(),
            keys: VecDeque::new(),
            fly: FlyoverState::new(),
            bios: FrameClock::bios(flyover::BIOS_PHASE_NUM, flyover::BIOS_PHASE_DEN),
            song_clock: FrameClock::song(),
            song_end: None,
            music_state: 0,
            sfx_end: 0,
            marks: Vec::new(),
            done: false,
            then: None,
        }
    }

    /// Loads the intro's files and builds the whole sequence.
    pub fn load(
        data: &DataDir,
        res: &ExeResources,
        options: IntroOptions,
    ) -> Result<Self, IntroError> {
        Ok(Self::new(IntroAssets::load(data, res)?, options))
    }

    /// The whole intro (intro §4.7–§4.9), starting at the date card.
    #[must_use]
    pub fn new(assets: IntroAssets, options: IntroOptions) -> Self {
        let mut s = Self::blank(Arc::new(assets.fly.clone()), options);
        // intro §4.7: intro0.scr's palette (four inks black) and pixels, before the date card.
        s.working = assets.intro0.palette.clone();
        for e in ZEROED_INKS {
            s.working.colors[usize::from(e)] = Rgb6::default();
        }
        s.vga.load_rows(0, WIDTH, &assets.intro0.pixels);
        let date_len = assets.narration[0].first().map_or(0, |t| t.chars().count()) as i32;
        let text = |set, index, pen, ink| Op::Text {
            set,
            index,
            pen,
            ink,
        };
        let card = |entry, ink, up_first: bool, hold, lines: Vec<Op>| {
            let mut ops = lines;
            ops.extend([
                Op::ColourFade {
                    entry,
                    ink,
                    up: up_first,
                },
                Op::Hold(hold),
                Op::ColourFade {
                    entry,
                    ink,
                    up: !up_first,
                },
            ]);
            ops
        };
        let mut ops = vec![
            // Date card (§4.7).
            Op::SetStart(START_DATE),
            text(0, 0, (160 - 5 * (date_len / 2), 280), 52),
            Op::PlayIntro1,
        ];
        ops.extend(card(52, Ink::Red, true, HOLD_DATE, vec![]));
        ops.push(Op::KeyCheck);
        // Narration on intro0.scr (§4.8).
        ops.extend([Op::SetStart(START_PLANET), Op::FullFade { up: true }]);
        ops.extend(ZEROED_INKS.map(|e| Op::SetDac(e, Rgb6::default())));
        for (entry, lines) in [
            (52, vec![text(0, 1, (20, 180), 52)]),
            (92, vec![text(0, 2, (27, 180), 92)]),
            (
                195,
                vec![text(0, 3, (12, 180), 195), text(0, 4, (55, 190), 195)],
            ),
        ] {
            ops.extend(card(entry, Ink::Cyan, true, HOLD_11, lines));
            ops.push(Op::KeyCheck);
        }
        ops.push(Op::FullFade { up: false });
        // The flyover (§4.6).
        ops.extend(Self::flyover_ops());
        ops.extend([
            Op::FullFade { up: false },
            Op::Delay(DELAY_AFTER_FLYOVER),
            Op::Intro2IfNeeded,
        ]);
        // intro1 (§4.9).
        ops.extend([
            Op::ClearPage(0),
            Op::SetStart(START_PAGE0),
            Op::LoadScr {
                pic: Pic::Intro1,
                row: 32,
                strip: 180,
            },
            Op::FullFade { up: true },
            Op::SetDac(45, Rgb6::default()),
        ]);
        ops.extend(card(
            45,
            Ink::Cyan,
            true,
            HOLD_10,
            vec![text(1, 0, (8, 212), 45)],
        ));
        ops.extend([Op::KeyCheck, Op::RestoreStrip(212)]);
        ops.extend(card(
            45,
            Ink::Cyan,
            true,
            HOLD_10,
            vec![text(1, 1, (8, 212), 45)],
        ));
        ops.extend([
            Op::RestoreStrip(212),
            Op::KeyCheck,
            Op::FullFade { up: false },
        ]);
        // intro2 (§4.9).
        let white = Rgb6 {
            r: 63,
            g: 63,
            b: 63,
        };
        ops.extend([
            Op::ClearPage(1),
            Op::SetStart(START_INTRO2),
            Op::LoadScr {
                pic: Pic::Intro2,
                row: 232,
                strip: 90,
            },
            Op::FullFade { up: true },
            Op::SetDac(140, white),
            Op::SetDac(141, white),
        ]);
        ops.extend(card(
            140,
            Ink::Grey,
            false,
            HOLD_11,
            vec![text(2, 0, (35, 322), 140)],
        ));
        ops.extend([Op::RestoreStrip(322), Op::KeyCheck]);
        ops.extend(card(
            141,
            Ink::Grey,
            false,
            HOLD_11,
            vec![text(2, 1, (17, 322), 141)],
        ));
        ops.push(Op::RestoreStrip(322));
        // Exit (§4.9), also the target of every Esc.
        s.exit_at = Some(ops.len());
        ops.extend([
            Op::FullFade { up: false },
            Op::Swallow,
            Op::StopMusic,
            Op::End,
        ]);
        s.script = ops;
        s.cards = Some(Arc::new(assets));
        s
    }

    /// The flyover alone, then a full fade out and the music stopped: the credits screen's
    /// mode A (`docs/spec/frontend/credits.md` rev 1 §4.1). Any key, Esc included, ends it
    /// after the current step.
    #[must_use]
    pub fn flyover_only(assets: FlyoverAssets, options: IntroOptions) -> Self {
        let mut s = Self::blank(Arc::new(assets), options);
        let mut ops = Self::flyover_ops();
        ops.extend([Op::FullFade { up: false }, Op::StopMusic, Op::End]);
        s.script = ops;
        s
    }

    fn flyover_ops() -> Vec<Op> {
        vec![
            Op::FlySetup,
            Op::SetStart(START_PAGE0),
            Op::FullFade { up: true },
            Op::SetDac(flyover::CAPTION_INK, Rgb6 { r: 0, g: 63, b: 63 }),
            Op::FlySteps,
        ]
    }

    /// Replaces this screen with `next` when it ends (instead of popping).
    #[must_use]
    pub fn then(mut self, next: Box<dyn Screen>) -> Self {
        self.then = Some(next);
        self
    }

    /// True once the sequence has ended.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.done
    }

    /// Frames run so far.
    #[must_use]
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// The flyover's variables.
    #[must_use]
    pub fn flyover(&self) -> &FlyoverState {
        &self.fly
    }

    /// The music state byte (`DS:0068`, intro §4.6): 1 while `intro.cmf` plays from the
    /// flyover setup, 2 from the date card's `intro1.cmf`, 3 once `intro2.cmf` was started.
    #[must_use]
    pub fn music_state(&self) -> u8 {
        self.music_state
    }

    /// What WORX's "digital sound playing" byte (fn 08h) reads now in the port's model: an
    /// effect started less than its measured busy time ago (intro rev 3 §3.6).
    #[must_use]
    pub fn effect_playing(&self) -> bool {
        2 * self.frame < self.sfx_end
    }

    /// What WORX's "music playing" byte (fn 0Eh) reads now in the port's model: a song was
    /// started and, if it plays with the loop off, has not reached its end yet (intro §4.6,
    /// U-370).
    #[must_use]
    pub fn music_playing(&self) -> bool {
        self.music_state != 0
            && self
                .song_end
                .is_none_or(|end| self.song_clock.ticks() < end)
    }

    /// The video memory.
    #[must_use]
    pub fn vga(&self) -> &VgaMemory {
        &self.vga
    }

    /// The DAC as shown.
    #[must_use]
    pub fn dac(&self) -> &Palette {
        &self.dac
    }

    /// The events so far.
    #[must_use]
    pub fn marks(&self) -> &[(u64, Mark)] {
        &self.marks
    }

    /// Advances one frame; audio commands go to `audio`.
    pub fn step(&mut self, input: &TickInput, audio: &mut Vec<AudioCommand>) {
        for &k in &input.pressed {
            if is_keystroke(k) && self.keys.len() < KEY_BUFFER {
                self.keys.push_back(k);
            }
        }
        if self.done {
            return;
        }
        self.frame += 1;
        self.retrace = true;
        self.song_clock.frame();
        loop {
            let Some(op) = self.script.get(self.pc).cloned() else {
                self.done = true;
                return;
            };
            match self.exec(&op, audio) {
                Next::Continue => {
                    self.pc += 1;
                    self.sub = None;
                }
                Next::Jump(to) => {
                    self.pc = to;
                    self.sub = None;
                }
                Next::Yield => return,
            }
            if self.done {
                return;
            }
        }
    }

    fn mark(&mut self, m: Mark) {
        self.marks.push((self.frame, m));
    }

    fn exec(&mut self, op: &Op, audio: &mut Vec<AudioCommand>) -> Next {
        match *op {
            Op::SetStart(start) => {
                if self.sub.is_none() {
                    self.sub = Some(0);
                    return Next::Yield;
                }
                self.vga.set_start(start);
            }
            Op::FullFade { up } => return self.full_fade(up),
            Op::ColourFade { entry, ink, up } => return self.colour_fade(entry, ink, up),
            Op::Hold(n) | Op::Delay(n) => {
                let until = *self.sub.get_or_insert(self.frame + u64::from(n));
                let key = matches!(op, Op::Hold(_)) && !self.keys.is_empty();
                if !key && self.frame < until {
                    return Next::Yield;
                }
            }
            Op::KeyCheck => {
                if self.keys.pop_front() == Some(Key::ESCAPE)
                    && let Some(exit) = self.exit_at
                {
                    return Next::Jump(exit);
                }
            }
            Op::SetDac(e, c) => self.dac.colors[usize::from(e)] = c,
            Op::Text {
                set,
                index,
                pen,
                ink,
            } => {
                if let Some(cards) = &self.cards
                    && let Some(t) = cards.narration[set].get(index)
                {
                    let font = &self.fly_assets.font;
                    draw::draw_text(&mut self.vga, font, TextStyle::B, t, pen, ink);
                }
            }
            Op::PlayIntro1 => {
                if let Some(song) = self.cards.as_ref().and_then(|c| c.intro1_song.clone()) {
                    self.music_state = 2;
                    audio.push(AudioCommand::PlayMusic {
                        song,
                        looping: true,
                    });
                    self.song_end = None;
                }
            }
            Op::LoadScr { pic, row, strip } => {
                if let Some(cards) = self.cards.clone() {
                    let scr = match pic {
                        Pic::Intro1 => &cards.intro1,
                        Pic::Intro2 => &cards.intro2,
                    };
                    self.working = scr.palette.clone();
                    self.vga.load_rows(row, WIDTH, &scr.pixels);
                    self.strip = scr.pixels[strip * WIDTH..(strip + STRIP_ROWS) * WIDTH].to_vec();
                }
            }
            Op::ClearPage(p) => {
                // From 0B00h (page 0's grid) or 5AC0h (page 1's origin), intro §4.2.
                let at = if p == 0 { START_PAGE0 } else { 0x5AC0 };
                self.vga.fill(at, CLEAR_ROWS * 88, 0);
            }
            Op::RestoreStrip(row) => {
                let strip = std::mem::take(&mut self.strip);
                self.vga.load_rows(row, WIDTH, &strip);
                self.strip = strip;
            }
            Op::FlySetup => self.fly_setup(audio),
            Op::FlySteps => return self.fly_steps(audio),
            Op::Intro2IfNeeded => {
                if self.music_state != 3 && self.fly_assets.intro_song.is_some() {
                    self.play_intro2(audio);
                }
            }
            Op::StopMusic => {
                audio.push(AudioCommand::StopMusic);
                self.song_end = None;
            }
            Op::Swallow => {
                self.keys.pop_front();
            }
            Op::End => {
                self.keys.clear();
                self.mark(Mark::End);
                self.done = true;
                return Next::Yield;
            }
        }
        Next::Continue
    }

    /// One level per frame; the op ends at the start of the frame after the last level.
    fn full_fade(&mut self, up: bool) -> Next {
        let i = self.sub.unwrap_or(0);
        if i >= u64::from(draw::FADE_LEVELS) {
            return Next::Continue;
        }
        if !self.retrace {
            return Next::Yield;
        }
        if i == 0 {
            self.mark(Mark::FullFade { up });
        }
        let level = if up { i as u8 } else { 64 - i as u8 };
        draw::full_fade_level(&mut self.dac, &self.working, level);
        self.retrace = false;
        self.sub = Some(i + 1);
        Next::Yield
    }

    /// Ten levels per frame; the code after it runs in the frame of the last levels.
    fn colour_fade(&mut self, entry: u8, ink: Ink, up: bool) -> Next {
        if !self.retrace {
            return Next::Yield;
        }
        let done = self.sub.unwrap_or(0);
        if done == 0 {
            self.mark(Mark::ColourFade { entry, up });
        }
        let end = (done + u64::from(COLOUR_LEVELS_PER_FRAME)).min(64);
        for n in done..end {
            let k = if up { n as u8 } else { 63 - n as u8 };
            self.dac.colors[usize::from(entry)] = ink.at(k);
        }
        self.retrace = false;
        if end == 64 {
            return Next::Continue;
        }
        self.sub = Some(end);
        Next::Yield
    }

    fn play_intro2(&mut self, audio: &mut Vec<AudioCommand>) {
        self.music_state = 3;
        audio.push(AudioCommand::StopMusic);
        match self.fly_assets.intro2_song.clone() {
            Some(song) => {
                self.song_end = Some(self.song_clock.ticks() + song_ticks(&song));
                audio.push(AudioCommand::PlayMusic {
                    song,
                    looping: false,
                });
            }
            None => self.song_end = None,
        }
    }

    /// intro §4.6 setup: `td.pal`, `intro.cmf` with the loop off, the variables, page 0's grid
    /// (columns 0..9).
    fn fly_setup(&mut self, audio: &mut Vec<AudioCommand>) {
        self.working = self.fly_assets.td_pal.clone();
        if let Some(song) = self.fly_assets.intro_song.clone() {
            audio.push(AudioCommand::StopMusic);
            audio.push(AudioCommand::SetMusicLoop(false));
            self.song_end = Some(self.song_clock.ticks() + song_ticks(&song));
            audio.push(AudioCommand::PlayMusic {
                song,
                looping: false,
            });
            self.music_state = 1;
        }
        self.fly = FlyoverState::new();
        self.sfx_end = 0;
        draw::draw_grid(&mut self.vga, &self.fly_assets.map, 0, self.fly.v194, 10);
    }

    /// The step loop (intro §4.6). Step s begins (draws its page) in the frame in which step
    /// s − 1's page is shown, the frame after a BIOS tick.
    fn fly_steps(&mut self, audio: &mut Vec<AudioCommand>) -> Next {
        if self.sub.is_none() {
            self.sub = Some(1);
            self.bios = FrameClock::bios(flyover::BIOS_PHASE_NUM, flyover::BIOS_PHASE_DEN);
            self.begin_step(audio);
            return Next::Yield;
        }
        if self.bios.frame() == 0 {
            return Next::Yield;
        }
        // 7..10: the limiter ends at the BIOS tick; set start shows the page; flip; motion.
        self.vga
            .set_start(draw::page_start(self.fly.page, self.fly.v198));
        self.fly.flip_and_move();
        // 11: a waiting key or the end leaves the loop.
        if self.keys.is_empty() && !self.fly.ended() {
            self.begin_step(audio);
            return Next::Yield;
        }
        if self.keys.pop_front() == Some(Key::ESCAPE)
            && let Some(exit) = self.exit_at
        {
            return Next::Jump(exit);
        }
        Next::Continue
    }

    /// Steps 1..6 of the loop: the music check, s + 1, the grid, the effects, the hoverskid and
    /// the caption.
    fn begin_step(&mut self, audio: &mut Vec<AudioCommand>) {
        let now = self.song_clock.ticks();
        if self.song_end.is_some_and(|end| now >= end) {
            self.play_intro2(audio);
        }
        self.fly.begin();
        self.mark(Mark::Step(self.fly.s));
        let assets = Arc::clone(&self.fly_assets);
        let page = self.fly.page;
        draw::draw_grid(
            &mut self.vga,
            &assets.map,
            page,
            self.fly.v194,
            draw::GRID_COLUMNS,
        );
        // "No effect playing" (intro rev 3 §3.6, U-380): the effect counts as playing for its
        // measured busy time from the frame it starts, in half frames.
        let half_frame = 2 * self.frame;
        let sfx_end = Cell::new(self.sfx_end);
        flyover::effects(
            &mut self.fly,
            || half_frame < sfx_end.get(),
            |e| {
                if let Some(sound) = assets.effect(e) {
                    sfx_end.set(half_frame + e.busy_half_frames());
                    audio.push(AudioCommand::PlaySfx {
                        sound: Arc::clone(sound),
                    });
                }
            },
        );
        self.sfx_end = sfx_end.get();
        if self.fly.y <= flyover::DRAW_LIMIT {
            draw::draw_hoverskid(
                &mut self.vga,
                &assets.sprites,
                page,
                flyover::HOVERSKID_SPRITE,
                self.fly.blit_y(),
                flyover::HOVERSKID_X,
                self.options.hoverskid(),
            );
        }
        if let Some((cx, row)) = flyover::caption_window(self.fly.s) {
            let text = match flyover::caption(self.fly.s) {
                Caption::Blank => None,
                Caption::Credit(k) => assets.credits.get(k),
            };
            if let Some(text) = text {
                let row = row + i32::from(self.fly.v198) + 264 * i32::from(page);
                draw::draw_caption(
                    &mut self.vga,
                    &assets.font,
                    text,
                    cx,
                    row,
                    flyover::CAPTION_INK,
                );
            }
        }
    }
}

/// How many song ticks `song` plays with the loop off: its end-of-track tick plus one (the
/// first event runs at tick d0 + 1, cmf §4.4; music-cues C3).
#[must_use]
pub fn song_ticks(song: &Cmf) -> u64 {
    u64::from(song.end_tick) + 1
}

impl Screen for Intro {
    fn tick(&mut self, ctx: &mut FlowContext<'_>, input: &TickInput) -> Transition {
        self.step(input, ctx.audio);
        if !self.done {
            return Transition::None;
        }
        match self.then.take() {
            Some(next) => Transition::Replace(next),
            None => Transition::Pop,
        }
    }

    fn draw(&self, fb: &mut Framebuffer) {
        self.vga.render(fb);
        fb.palette.clone_from(&self.dac);
    }
}

#[cfg(test)]
mod tests;
