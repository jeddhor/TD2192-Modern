//! The real-time screens that Episode 1's mission events show from inside a tick: flight 17's
//! cut-scene (`td1-17.pic` and five lines with DAC flashes) and flight 19's V.C.I. screen.
//!
//! Spec: `docs/spec/flow/flight-screens.md` rev 4 §4.10 (BUG-3), `docs/spec/sim/mission-logic.md`
//! rev 3 §5.17, §5.19, `docs/spec/sim/hit-flash.md` rev 4 §4.2. Fidelity: T0 for the pictures
//! and DAC values (C-0067, C-0069), T2 for the pacing (`Crt.Delay` real time, the fade speed of
//! the reference machine).
//!
//! The original runs each screen to completion inside step 18 of the tick that requests it:
//! no tick passes, no key is read, music goes on. Both use the flight fades
//! ([`fade_target`], 17 levels), which also end any hit flash; td-sim zeroes `g_HitFlash` when
//! it reports the request. The port runs the screen as a queue of steps, one vertical retrace
//! per waiting step, like the computer menu ([`super::menu`]).
//!
//! The screen shows page 0 from its origin: first with the split line off (no HUD; rows 0..191
//! of the screen are page rows 0..191), and for the closing fade-in with the split line back
//! at row 160, so the HUD band as the flight last drew it shows below a black playfield
//! (C-0067 frame 1520).

use std::collections::VecDeque;

use td_formats::palette::{Palette, Rgb6};
use td_render::flight_palette::{FADE_STEPS, fade_target, write_fade_step};
use td_render::{Framebuffer, WIDTH, small_text};

use super::{FLIGHT_ROWS, FlightScreenAssets, ModalStep, blank_below_display};
use crate::screen::RETRACE_HZ;

/// Retraces per fade level: a level waits for a retrace, then writes 255 or 256 DAC entries
/// with three Real48 `Round`s each, which at preset P1 takes past the next retrace
/// (flight-screens §4.10.1: "four 17-level fades (about 34 frames each at P1)"; C-0067
/// `frames.tsv` 877..910). T2.
pub const FRAMES_PER_FADE_LEVEL: u32 = 2;

/// Retraces the original spends reading `td1-17.pic` one byte at a time (C-0067 frames
/// 911..925, 28,800 one-byte reads at P1). The picture is drawn while the DAC is black, so only
/// the pacing depends on it. T2.
pub const PICTURE_LOAD_FRAMES: u32 = 15;

/// Screen rows of the cut-scene picture: page rows 35..124 (VGA rows 67..156).
pub const PICTURE_TOP: usize = 35;
/// `td1-17.pic`: 320 × 90 bytes, no palette, read from offset 0.
pub const PICTURE_ROWS: usize = 90;

/// Screen pens of the cut-scene's five lines (text y 172, 182, .. 212 minus VGA row 32) and
/// their inks (`e1.flight_events.02[0..4]`).
pub const CUT_SCENE_LINES: [((i32, i32), u8); 5] = [
    ((10, 140), 131),
    ((10, 150), 248),
    ((10, 160), 131),
    ((10, 170), 248),
    ((10, 180), 250),
];
/// `Crt.Delay` after each cut-scene line, in milliseconds (the DAC flash follows line 0's).
const CUT_SCENE_DELAYS_MS: [u32; 5] = [1000, 1000, 1000, 1000, 2000];
/// The three text inks, which the flash loops spare.
const TEXT_INKS: [usize; 3] = [131, 248, 250];
/// The second flash loop runs to entry 145 (91h) only, sparing ink 131 alone.
const FLASH_LAST: usize = 145;
/// The second flash loop's colour.
const FLASH_RGB: Rgb6 = Rgb6 {
    r: 63,
    g: 63,
    b: 13,
};

/// The V.C.I. screen's line (`e1.flight_events.01[5]`): screen pen (text (40, 80)) and ink.
pub const VCI_LINE: ((i32, i32), u8) = ((40, 48), 131);
/// Index of that line in `e1.flight_events.01`.
const VCI_TEXT: usize = 5;
/// How long the V.C.I. line stays (`Crt.Delay(2000)`).
const VCI_DELAY_MS: u32 = 2000;

/// Rows of the HUD band shown below the playfield when the split line is at 13Fh.
const HUD_TOP: usize = 160;

/// `Crt.Delay(ms)` in retraces (T2).
fn delay_frames(ms: u32) -> u32 {
    (f64::from(ms) * RETRACE_HZ / 1000.0).round() as u32
}

/// One step of a screen's program.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    /// One fade level per [`FRAMES_PER_FADE_LEVEL`] retraces: DAC entries `first..` :=
    /// Round(`target` · s / 16), s moving from `s` to `end`.
    Fade {
        target: Box<Palette>,
        s: u8,
        end: u8,
        first: usize,
    },
    /// Wait this many retraces, the current one included.
    Hold(u32),
    /// Clear page 0 and show it from its origin; `hud`: with the split line at 13Fh, so the
    /// HUD band shows below row 160.
    ClearPage { hud: bool },
    /// Plot `td1-17.pic` at screen rows 35..124.
    Picture,
    /// Draw a line with the flight font, transparent.
    Text {
        text: String,
        pen: (i32, i32),
        ink: u8,
    },
    /// `SetDac(e, rgb)` for e = 0..=`last` except `skip`; `None`: `g_FlightPalette[e]`.
    Dac {
        last: usize,
        skip: &'static [usize],
        rgb: Option<Rgb6>,
    },
}

/// A real-time screen of the mission events (flight-screens §4.10).
#[derive(Debug, Clone)]
pub struct Interlude {
    canvas: Framebuffer,
    /// Rows 160..191 of the frame on screen when the screen opened: the HUD as the flight last
    /// drew it (VGA rows 0..31), which the closing fade-in shows again.
    hud_band: Vec<u8>,
    /// `g_FlightPalette` (the flight's palette with its lamp entries).
    flight_palette: Palette,
    /// `td1-17.pic` for the cut-scene (empty for the V.C.I. screen).
    picture: Vec<u8>,
    ops: VecDeque<Op>,
    /// Retraces run so far.
    frames: u32,
}

/// The mission-logic request of `scene` ([`td_sim::flight::logic::MissionState::scene`]).
pub const SCENE_CUT_SCENE: u8 = 17;
/// The V.C.I. screen request.
pub const SCENE_VCI: u8 = 19;

impl Interlude {
    /// The screen for request `scene` (17 or 19), over `shown` (the frame on screen, whose DAC
    /// is the flight's), with the flight palette `flight_palette` (`g_FlightPalette`) and the
    /// palette mode (0 in flights 17 and 19). `None` for another request or when the assets
    /// lack the screen's data.
    #[must_use]
    pub fn new(
        scene: u8,
        shown: Framebuffer,
        flight_palette: &Palette,
        mode: u8,
        assets: &FlightScreenAssets,
    ) -> Option<Self> {
        let target = fade_target(flight_palette, mode);
        let fade_out = || Op::Fade {
            target: Box::new(target.clone()),
            s: FADE_STEPS,
            end: 0,
            first: 0,
        };
        let fade_in = || Op::Fade {
            target: Box::new(target.clone()),
            s: 0,
            end: FADE_STEPS,
            first: 1,
        };
        let text = |list: &[String], i: usize| list.get(i).cloned().unwrap_or_default();
        let mut ops = VecDeque::new();
        // The common frame's start (§4.10): FadeOut, clear page 0, split line 400, page 0
        // from its origin.
        ops.push_back(fade_out());
        ops.push_back(Op::ClearPage { hud: false });
        let mut picture = Vec::new();
        match scene {
            SCENE_CUT_SCENE => {
                if assets.scene_picture.len() < PICTURE_ROWS * WIDTH {
                    return None;
                }
                picture = assets.scene_picture[..PICTURE_ROWS * WIDTH].to_vec();
                // §4.10.1.
                ops.push_back(Op::Hold(PICTURE_LOAD_FRAMES));
                ops.push_back(Op::Picture);
                ops.push_back(fade_in());
                for (i, (&(pen, ink), &ms)) in
                    CUT_SCENE_LINES.iter().zip(&CUT_SCENE_DELAYS_MS).enumerate()
                {
                    ops.push_back(Op::Text {
                        text: text(&assets.scene_texts, i),
                        pen,
                        ink,
                    });
                    ops.push_back(Op::Hold(delay_frames(ms)));
                    if i == 0 {
                        ops.extend(flash());
                        ops.push_back(Op::Hold(delay_frames(1000)));
                    }
                }
            }
            SCENE_VCI => {
                // §4.10.2: the line is drawn while the DAC is black.
                let (pen, ink) = VCI_LINE;
                ops.push_back(Op::Text {
                    text: text(&assets.vci_texts, VCI_TEXT),
                    pen,
                    ink,
                });
                ops.push_back(fade_in());
                ops.push_back(Op::Hold(delay_frames(VCI_DELAY_MS)));
            }
            _ => return None,
        }
        // The common frame's end: FadeOut, split line 13Fh, clear page 0, (flight 19: the
        // vehicle switch, done by td-sim), FadeIn.
        ops.push_back(fade_out());
        ops.push_back(Op::ClearPage { hud: true });
        ops.push_back(fade_in());
        let band = HUD_TOP * WIDTH..FLIGHT_ROWS * WIDTH;
        Some(Self {
            hud_band: shown
                .pixels
                .get(band)
                .map(<[u8]>::to_vec)
                .unwrap_or_default(),
            canvas: shown,
            flight_palette: flight_palette.clone(),
            picture,
            ops,
            frames: 0,
        })
    }

    /// The picture and DAC on screen.
    pub fn canvas(&self) -> &Framebuffer {
        &self.canvas
    }

    /// Retraces run so far.
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Advances one retrace. The screen reads no keys.
    pub fn tick(&mut self, assets: &FlightScreenAssets) -> ModalStep {
        self.frames += 1;
        while let Some(op) = self.ops.pop_front() {
            match op {
                Op::Fade {
                    target,
                    s,
                    end,
                    first,
                } => {
                    write_fade_step(&mut self.canvas.palette, &target, s, first);
                    if s != end {
                        let s = if s < end { s + 1 } else { s - 1 };
                        self.ops.push_front(Op::Fade {
                            target,
                            s,
                            end,
                            first,
                        });
                    }
                    self.ops.push_front(Op::Hold(FRAMES_PER_FADE_LEVEL));
                }
                Op::Hold(n) => {
                    if n > 1 {
                        self.ops.push_front(Op::Hold(n - 1));
                    }
                    return ModalStep::Continue;
                }
                Op::ClearPage { hud } => {
                    self.canvas.pixels.fill(0);
                    if hud {
                        let band = HUD_TOP * WIDTH..HUD_TOP * WIDTH + self.hud_band.len();
                        self.canvas.pixels[band].copy_from_slice(&self.hud_band);
                    }
                    blank_below_display(&mut self.canvas);
                }
                Op::Picture => {
                    let at = PICTURE_TOP * WIDTH;
                    self.canvas.pixels[at..at + self.picture.len()].copy_from_slice(&self.picture);
                }
                Op::Text { text, pen, ink } => {
                    small_text::draw(&mut self.canvas, &assets.font, &text, pen, ink);
                }
                Op::Dac { last, skip, rgb } => {
                    for e in (0..=last).filter(|e| !skip.contains(e)) {
                        self.canvas.palette.colors[e] =
                            rgb.unwrap_or(self.flight_palette.colors[e]);
                    }
                }
            }
        }
        ModalStep::Close { abort: false }
    }
}

/// The cut-scene's DAC flash after line 0 (§4.10.1): black, yellow-white up to entry 145,
/// black, then the flight palette, each loop followed by `Crt.Delay(100)` but the last; the
/// text inks are spared (the second loop spares only 131, but 248 and 250 lie above 145).
fn flash() -> Vec<Op> {
    let black = Some(Rgb6::default());
    let wait = || Op::Hold(delay_frames(100));
    vec![
        Op::Dac {
            last: 255,
            skip: &TEXT_INKS,
            rgb: black,
        },
        wait(),
        Op::Dac {
            last: FLASH_LAST,
            skip: &TEXT_INKS[..1],
            rgb: Some(FLASH_RGB),
        },
        wait(),
        Op::Dac {
            last: 255,
            skip: &TEXT_INKS,
            rgb: black,
        },
        wait(),
        Op::Dac {
            last: 255,
            skip: &TEXT_INKS,
            rgb: None,
        },
    ]
}
