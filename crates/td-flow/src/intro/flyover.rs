//! The intro flyover's state: scroll, hoverskid motion, the `Random` weave, the caption table,
//! the sound-effect schedule and the clocks that pace the steps.
//!
//! Spec: `docs/spec/flow/intro.md` (rev 1) §3.5 (captions), §3.6 (effects, U-036), §3.7
//! (constants), §4.3 (timing) and §4.6 (the loop). Fidelity: T1 for the state per step (C-0047
//! RAM, `tests/intro_parity.rs`), T2 for the pacing and the audio.

use td_sim::rng::BpRandom;

/// The step value that ends the loop (intro §2).
pub const END_STEP: u16 = 10_000;
/// Hoverskid sprite: `td.shp` record 4 (intro §3.7).
pub const HOVERSKID_SPRITE: usize = 4;
/// Hoverskid x (drawn at the 4-pixel boundary 140, intro §3.7).
pub const HOVERSKID_X: i16 = 142;
/// The hoverskid is drawn only while y ≤ 232 (signed, intro §3.7).
pub const DRAW_LIMIT: i16 = 232;
/// y ≥ 292 ends the flyover (intro §3.7).
pub const END_Y: i16 = 292;
/// Caption ink: DAC 46 = (0, 63, 63) after the fade-in (intro §3.4).
pub const CAPTION_INK: u8 = 46;

/// The loop's variables (intro §4.6). `steer`, `speed` and `y` are Pascal Integers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlyoverState {
    /// Step `DS:0190`: 0 before the first step, 1..=1618, then [`END_STEP`].
    pub s: u16,
    /// First map row on screen, `DS:0194` (starts at 2).
    pub v194: i16,
    /// Vertical offset `DS:0198` (16..47); the screen shows the grid from row v198 − 16.
    pub v198: i16,
    /// Hoverskid y (`DS:AA40`, starts at −32).
    pub y: i16,
    pub steer: i16,
    pub speed: i16,
    /// The page drawn on (0 or 1).
    pub page: u8,
    /// The effect flag `DS:0069` (§3.6).
    pub flag: bool,
    /// `RandSeed`: the intro never calls `Randomize`, so it starts at 0.
    pub rng: BpRandom,
}

impl Default for FlyoverState {
    fn default() -> Self {
        Self::new()
    }
}

impl FlyoverState {
    /// The state after the setup (intro §4.6): page 0, v194 2, v198 16, y −32.
    #[must_use]
    pub fn new() -> Self {
        Self {
            s: 0,
            v194: 2,
            v198: 16,
            y: -32,
            steer: 0,
            speed: 0,
            page: 0,
            flag: false,
            rng: BpRandom::new(),
        }
    }

    /// True once step 1618 (or a key) ended the loop.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.s == END_STEP
    }

    /// Step 2: s := s + 1.
    pub fn begin(&mut self) {
        self.s += 1;
    }

    /// The y passed to the blit at step 5: y + v198 + 32 × page.
    #[must_use]
    pub fn blit_y(&self) -> i16 {
        self.y + self.v198 + 32 * i16::from(self.page)
    }

    /// Steps 9 and 10 after the flip: page := 1 − page, then the motion rules in order
    /// (intro §4.6). One `Random(3)` per step for 37 < s < 1605.
    pub fn flip_and_move(&mut self) {
        self.page = 1 - self.page;
        let s = self.s as i16;
        if 19 < s && s <= 37 {
            self.y += 37 - s;
            self.v198 += s - 20;
        }
        if 37 < s && s < 1600 {
            self.v198 += 17;
        }
        if s >= 1600 {
            if s <= 1617 {
                self.v198 += 1617 - s;
            }
            self.y += s - 1600;
            if self.y >= END_Y {
                self.s = END_STEP;
            }
        }
        if 37 < s && s < 1605 {
            self.steer = 1 - self.rng.random(3) as i16;
            if self.y < 65 {
                self.steer = 1;
            }
            if self.y > 145 {
                self.steer = -1;
            }
            self.speed += self.steer;
            if self.speed > 5 {
                self.speed = 2;
            }
            if self.speed < -5 {
                self.speed = -2;
            }
            self.y += self.speed;
        }
        if self.v198 >= 48 {
            self.v194 += 1;
            self.v198 -= 32;
        }
    }
}

/// A caption choice: blank, or `intro.credits[k]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caption {
    Blank,
    Credit(usize),
}

/// The caption table (intro §3.5): the last row whose threshold s exceeds wins.
const CAPTIONS: [(u16, Caption); 37] = {
    use Caption::{Blank as B, Credit as C};
    [
        (70, C(0)),
        (150, B),
        (190, C(1)),
        (270, B),
        (300, C(2)),
        (400, B),
        (440, C(3)),
        (500, C(4)),
        (550, C(5)),
        (600, C(6)),
        (650, B),
        (700, C(7)),
        (750, C(4)),
        (790, C(8)),
        (830, C(9)),
        (870, C(10)),
        (910, B),
        (950, C(11)),
        (1000, C(5)),
        (1050, B),
        (1100, C(12)),
        (1150, C(13)),
        (1200, B),
        (1250, C(14)),
        (1300, C(15)),
        (1310, C(16)),
        (1320, C(17)),
        (1330, C(18)),
        (1340, C(19)),
        (1350, C(20)),
        (1360, C(21)),
        (1370, C(22)),
        (1380, C(23)),
        (1420, C(24)),
        (1470, C(4)),
        (1530, B),
        // Sentinel never reached by s (keeps the table a fixed size).
        (u16::MAX, B),
    ]
};

/// The caption text chosen at step `s` (intro §3.5).
#[must_use]
pub fn caption(s: u16) -> Caption {
    CAPTIONS
        .iter()
        .filter(|&&(t, _)| s > t)
        .last()
        .map_or(Caption::Blank, |&(_, c)| c)
}

/// Where the caption is drawn at step `s`: centre x and the row relative to the scrolled page
/// (screen row + 16), or `None` outside every window (intro §3.5).
#[must_use]
pub fn caption_window(s: u16) -> Option<(i32, i32)> {
    match s {
        _ if !(400..=1420).contains(&s) => Some((160, 100)),
        441..=649 => Some((250, 150)),
        701..=899 => Some((80, 60)),
        951..=1049 => Some((250, 40)),
        1101..=1199 => Some((80, 70)),
        1251..=1379 => Some((250, 130)),
        _ => None,
    }
}

/// The five intro effects (intro §3.1, §3.6; U-036).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Whoosh,
    Pulse,
    Ric3,
    Scratch,
    Home,
}

impl Effect {
    /// All effects, in the load order of intro §3.1.
    pub const ALL: [Effect; 5] = [
        Effect::Whoosh,
        Effect::Pulse,
        Effect::Ric3,
        Effect::Scratch,
        Effect::Home,
    ];

    /// The voice file.
    #[must_use]
    pub fn file(self) -> &'static str {
        match self {
            Effect::Whoosh => "whoosh.voc",
            Effect::Pulse => "pulse.voc",
            Effect::Ric3 => "ric3.voc",
            Effect::Scratch => "scratch.voc",
            Effect::Home => "home.voc",
        }
    }

    /// The SB Pro voice level set with it (`WORX_SetVoiceVolume(v, v)`, of 15). Not applied by
    /// the port's mixer yet (T2).
    #[must_use]
    pub fn volume(self) -> u8 {
        match self {
            Effect::Ric3 | Effect::Home => 13,
            Effect::Pulse => 10,
            Effect::Whoosh | Effect::Scratch => 12,
        }
    }
}

/// The effect checks of one step (intro §3.6), run after the grid is drawn. `playing` reports
/// whether an effect is still sounding; `play` starts one (which stops the playing one).
pub fn effects(
    state: &mut FlyoverState,
    mut playing: impl FnMut() -> bool,
    mut play: impl FnMut(Effect),
) {
    // The caller's `play` changes what `playing` reports, so every rule sees the effects
    // started by the rules before it (rule 8 waits on the step rule 4 or 7 fires).
    let s = state.s;
    if s == 1 {
        play(Effect::Whoosh);
    }
    if s == 150 {
        play(Effect::Scratch);
    }
    if s == 155 {
        play(Effect::Ric3);
    }
    if 155 < s && s < 240 && !playing() && !state.flag {
        state.flag = true;
        play(Effect::Scratch);
    }
    if s == 240 {
        play(Effect::Scratch);
        state.flag = false;
    }
    if s == 245 {
        play(Effect::Home);
    }
    if 265 < s && s < 330 && !playing() && !state.flag {
        state.flag = true;
        play(Effect::Scratch);
    }
    if !playing() {
        play(Effect::Pulse);
    }
}

/// PIT input clock over the song divisor: a 96 Hz CMF song sets PIT channel 0 to
/// 1,193,180 div 96 = 12,428 (cmf §4.1, timing §3).
pub const PIT_CLOCK: u64 = 1_193_180;
/// The song divisor of every shipped song.
pub const SONG_DIVISOR: u64 = 12_428;
/// WORX calls the BIOS tick every 65535 div 12428 = 5 song ticks (timing §4.2): the flyover
/// steps once per BIOS tick, about 19.20 Hz.
pub const PIT_PER_BIOS: u64 = 5;
/// The retrace rate in millihertz (70.086 Hz).
pub const RETRACE_MILLIHZ: u64 = 70_086;

/// Timer ticks counted per 70.086 Hz frame with an exact rational accumulator: the song (PIT)
/// clock at 1,193,180 / 12,428 Hz, or the BIOS tick at a fifth of that (timing §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameClock {
    acc: u64,
    ticks: u64,
    per_tick: u64,
}

impl FrameClock {
    const PER_FRAME: u64 = PIT_CLOCK * 1000;

    /// The song clock (96.0074 Hz), at tick 0.
    #[must_use]
    pub fn song() -> Self {
        Self {
            acc: 0,
            ticks: 0,
            per_tick: SONG_DIVISOR * RETRACE_MILLIHZ,
        }
    }

    /// The BIOS tick while a song plays (19.2015 Hz), at tick 0, with `num / den` of a tick
    /// already elapsed.
    #[must_use]
    pub fn bios(num: u64, den: u64) -> Self {
        let per_tick = SONG_DIVISOR * PIT_PER_BIOS * RETRACE_MILLIHZ;
        Self {
            acc: per_tick * num / den.max(1) % per_tick,
            ticks: 0,
            per_tick,
        }
    }

    /// Advances one frame; returns the ticks that elapsed.
    pub fn frame(&mut self) -> u64 {
        self.acc += Self::PER_FRAME;
        let n = self.acc / self.per_tick;
        self.acc %= self.per_tick;
        self.ticks += n;
        n
    }

    /// Ticks since the clock started.
    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.ticks
    }
}

/// Part of a BIOS tick already elapsed when the first step starts (numerator over
/// [`BIOS_PHASE_DEN`]). ASSUMPTION: chosen so that C-0047's cadence is reproduced (steps 2, 3,
/// 4, 5 start 3, 7, 10, 14 frames after step 1); the original's phase depends on when its
/// loading ended (T2).
pub const BIOS_PHASE_NUM: u64 = 137;
/// Denominator of [`BIOS_PHASE_NUM`].
pub const BIOS_PHASE_DEN: u64 = 400;

/// Converts `samples` at `rate` Hz into song ticks, rounded up.
#[must_use]
pub fn samples_to_ticks(samples: usize, rate: u32) -> u64 {
    let num = samples as u64 * PIT_CLOCK;
    let den = u64::from(rate.max(1)) * SONG_DIVISOR;
    num.div_ceil(den)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs steps 1..=n of the motion with no key.
    fn run(n: u16) -> FlyoverState {
        let mut st = FlyoverState::new();
        for _ in 0..n {
            st.begin();
            st.flip_and_move();
            if st.ended() {
                break;
            }
        }
        st
    }

    fn tuple(st: &FlyoverState) -> (i16, i16, i16, i16, i16, u32) {
        (st.v194, st.v198, st.y, st.steer, st.speed, st.rng.seed())
    }

    #[test]
    fn spec_vector_4_state_after_steps() {
        // intro §6 vector 4: (v194, v198, y, steer, speed, RandSeed) after step s.
        assert_eq!(tuple(&run(20)), (2, 16, -15, 0, 0, 0));
        let st = run(21);
        assert_eq!((st.v194, st.v198, st.y), (2, 17, 1));
        assert_eq!(tuple(&run(37)), (6, 41, 121, 0, 0, 0));
        assert_eq!(tuple(&run(38)), (7, 26, 122, 1, 1, 1));
        assert_eq!(tuple(&run(39)), (7, 43, 124, 1, 2, 134_775_814));
        assert_eq!(tuple(&run(40)), (8, 28, 125, -1, 1, 3_698_175_007));
        assert_eq!(tuple(&run(100)), (40, 24, 131, 0, -1, 2_127_203_315));
        assert_eq!(tuple(&run(500)), (252, 40, 126, 0, 1, 2_815_784_867));
        assert_eq!(tuple(&run(1000)), (518, 28, 87, 0, -1, 2_302_979_935));
        assert_eq!(tuple(&run(1604)), (838, 46, 135, 0, 1, 1_689_385_875));
        let st = run(1617);
        assert_eq!((st.v194, st.v198, st.y), (841, 28, 278));
        let st = run(1618);
        assert_eq!((st.y, st.s), (296, END_STEP));
    }

    #[test]
    fn exactly_1567_random_calls() {
        let mut probe = BpRandom::new();
        for _ in 0..1567 {
            probe.step();
        }
        assert_eq!(run(1618).rng.seed(), probe.seed(), "steps 38..1604");
    }

    #[test]
    fn caption_table_and_windows() {
        assert_eq!(caption(70), Caption::Blank);
        assert_eq!(caption(71), Caption::Credit(0));
        assert_eq!(caption(1000), Caption::Credit(11));
        assert_eq!(caption(1001), Caption::Credit(5));
        assert_eq!(caption(1381), Caption::Credit(23));
        assert_eq!(caption(1471), Caption::Credit(4));
        assert_eq!(caption(1531), Caption::Blank);
        assert_eq!(caption_window(399), Some((160, 100)));
        assert_eq!(caption_window(400), None);
        assert_eq!(caption_window(441), Some((250, 150)));
        assert_eq!(caption_window(650), None);
        assert_eq!(caption_window(1379), Some((250, 130)));
        assert_eq!(caption_window(1420), None);
        assert_eq!(caption_window(1421), Some((160, 100)));
    }

    #[test]
    fn effect_schedule() {
        // A model where every effect lasts `len` steps.
        fn schedule(len: u16) -> Vec<(u16, Effect)> {
            let mut st = FlyoverState::new();
            let mut ends = 0u16;
            let mut out = Vec::new();
            for s in 1..=400u16 {
                st.s = s;
                let started = std::cell::RefCell::new(Vec::new());
                let end = std::cell::Cell::new(ends);
                effects(
                    &mut st,
                    || s < end.get(),
                    |e| {
                        started.borrow_mut().push(e);
                        end.set(s + len);
                    },
                );
                ends = end.get();
                out.extend(started.into_inner().into_iter().map(|e| (s, e)));
            }
            out
        }
        let sc = schedule(30);
        let non_pulse: Vec<_> = sc.iter().filter(|(_, e)| *e != Effect::Pulse).collect();
        // whoosh at 1; scratch 150; ric3 155; rule 4 when ric3 ends (185); scratch 240; home
        // 245; rule 7 on the first silent step after 265 (275).
        assert_eq!(
            non_pulse,
            [
                &(1, Effect::Whoosh),
                &(150, Effect::Scratch),
                &(155, Effect::Ric3),
                &(185, Effect::Scratch),
                &(240, Effect::Scratch),
                &(245, Effect::Home),
                &(275, Effect::Scratch),
            ]
        );
        // pulse fills every silence, never on a step where another effect started.
        assert!(sc.contains(&(31, Effect::Pulse)));
        assert!(!sc.contains(&(185, Effect::Pulse)));
    }

    #[test]
    fn song_clock_is_96_hz_and_bios_19_2() {
        let frames = 70_086; // 1000 s
        let mut c = FrameClock::song();
        let ticks: u64 = (0..frames).map(|_| c.frame()).sum();
        // 1,193,180 / 12,428 = 96.0074 Hz.
        assert_eq!(ticks, 96_007);
        assert_eq!(c.ticks(), ticks);
        let mut b = FrameClock::bios(0, 1);
        let ticks: u64 = (0..frames).map(|_| b.frame()).sum();
        assert_eq!(ticks, 19_201); // 96,007.4 / 5
        // Steps are 3 or 4 frames apart.
        let mut b = FrameClock::bios(BIOS_PHASE_NUM, BIOS_PHASE_DEN);
        let ticks: Vec<u32> = (1..=1000).filter(|_| b.frame() > 0).collect();
        // Steps 2.. start 3, 7, 10, 14 frames after step 1 (C-0047 f1618..1632).
        assert_eq!(&ticks[..4], &[3, 7, 10, 14]);
        assert!(ticks.windows(2).all(|w| (3..=4).contains(&(w[1] - w[0]))));
        assert_eq!(samples_to_ticks(11_025, 11_025), 97);
    }
}
