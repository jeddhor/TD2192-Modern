//! The E1 mission blocks around flights 19, 20 and 21 (`docs/spec/flow/episode-program-flow.md`
//! rev 5 §4.3, §4.4; `screens.md` rev 2 §3.12; `audio/music-cues.md` rev 2): mission 20 has no
//! `low` cue and no debrief (U-207); in block 19 Esc before flight 21 jumps to flight 21; after
//! flight 21 only the abort flag counts (U-081). Placeholder flights stand in for the real ones
//! (any key: success, `a`: aborted, `d`: died). Needs TD2192_DATA (story files); skips
//! otherwise.

use std::cell::RefCell;
use std::rc::Rc;

use td_assets::{DataDir, ExeResources};
use td_audio::AudioCommand;
use td_flow::campaign::{BlockResult, Campaign};
use td_flow::program::Program;
use td_flow::settings::Settings;
use td_flow::{FlightLauncher, FlightScreen, FlowContext, PlaceholderLauncher};
use td_input::{Key, TickInput};
use td_test_support::require_data;

/// Records the flights a block launches.
#[derive(Debug)]
struct Recorder {
    flights: Rc<RefCell<Vec<u8>>>,
    inner: PlaceholderLauncher,
}

impl FlightLauncher for Recorder {
    fn launch(&mut self, ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        self.flights.borrow_mut().push(flight);
        self.inner.launch(ctx, flight)
    }

    fn random(&mut self, n: u16) -> u16 {
        self.inner.random(n)
    }
}

/// What happened in one frame of a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    /// Flights launched so far.
    launches: usize,
    in_flight: bool,
    debrief: bool,
    retry: bool,
    /// The music cue playing (`MusicCues::current`).
    cue: Option<u8>,
}

/// Plays block `mission`. Every 40 frames it presses `key(state, last flight launched)`.
fn play(
    mission: u8,
    key: impl Fn(&Frame, Option<u8>) -> Option<Key>,
) -> (BlockResult, Vec<Frame>, Vec<u8>) {
    let data = td_test_support::data_dir().unwrap();
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    let flights = Rc::new(RefCell::new(Vec::new()));
    let launcher = Recorder {
        flights: Rc::clone(&flights),
        inner: PlaceholderLauncher::default(),
    };
    let mut prog = Program::new(&dir, &res, Box::new(launcher)).unwrap();
    let settings = Settings::default();
    let mut audio: Vec<AudioCommand> = Vec::new();
    let mut c = Campaign::new(mission, 0);
    let mut frames = Vec::new();
    for n in 0..400_000u32 {
        let state = Frame {
            launches: flights.borrow().len(),
            in_flight: c.in_flight(),
            debrief: c.in_debrief(),
            retry: c.in_retry_prompt(),
            cue: prog.cues.current(),
        };
        frames.push(state);
        let last = flights.borrow().last().copied();
        let pressed = if n.is_multiple_of(40) {
            key(&state, last).into_iter().collect()
        } else {
            Vec::new()
        };
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput {
            pressed,
            ..TickInput::default()
        };
        if let Some(r) = c.step(&mut prog, &mut ctx, &input) {
            let launched = flights.borrow().clone();
            return (r, frames, launched);
        }
        audio.clear();
    }
    panic!("block {mission} never ended");
}

/// The cues started after the frame `from`, in order (consecutive repeats merged).
fn cues_from(frames: &[Frame], from: usize) -> Vec<Option<u8>> {
    let mut out: Vec<Option<u8>> = Vec::new();
    for f in &frames[from..] {
        if out.last() != Some(&f.cue) {
            out.push(f.cue);
        }
    }
    out
}

/// The first frame after the k-th flight launched (1-based) ended (the number of frames when
/// the block ended with the flight).
fn after_flight(frames: &[Frame], k: usize) -> usize {
    let start = frames
        .iter()
        .position(|f| f.launches == k && f.in_flight)
        .unwrap();
    frames[start..]
        .iter()
        .position(|f| !f.in_flight)
        .map_or(frames.len(), |i| start + i)
}

fn space(_: &Frame, _: Option<u8>) -> Option<Key> {
    Some(Key::SPACE)
}

#[test]
fn block_20_has_no_low_cue_and_no_debrief() {
    // episode-program-flow rev 5 §4.4 (U-207): after a successful flight 20 the block goes
    // straight to the post-flight story (`mood3`, cue 13); no `low` (cue 8), no debrief.
    let _data = require_data!();
    let (r, frames, flights) = play(20, space);
    assert_eq!(flights, [20]);
    assert!(r.advance && !r.game_over, "{r:?}");
    let after = after_flight(&frames, 1);
    assert!(!frames.iter().any(|f| f.debrief), "no debrief in block 20");
    let cues = cues_from(&frames, after);
    assert!(
        !cues.contains(&Some(8)),
        "no low cue after flight 20: {cues:?}"
    );
    assert_eq!(cues.iter().flatten().next(), Some(&13), "{cues:?}");
    // Control: block 18 has the debrief, with `low`.
    let (_, frames, _) = play(18, space);
    let after = after_flight(&frames, 1);
    assert!(frames[after..].iter().any(|f| f.debrief));
    assert_eq!(cues_from(&frames, after).iter().flatten().next(), Some(&8));
}

#[test]
fn block_19_esc_before_flight_21_jumps_to_flight_21() {
    // screens rev 2 §3.12 (U-081): Esc on the C:voffice card after flight 19's debrief skips
    // D88, the M3 hangar card and D89 and starts flight 21; after flight 21 the block plays on.
    let _data = require_data!();
    let key = |f: &Frame, last: Option<u8>| {
        // Only Esc once flight 19 is over: keys typed during the debrief stay in the BIOS
        // buffer (screens §2.2), so a Space there would end the voffice card before an Esc
        // reached it. The debrief takes one Esc; the next ends the voffice card's wait.
        let esc = last == Some(19) && !f.in_flight;
        Some(if esc { Key::ESCAPE } else { Key::SPACE })
    };
    let (r, frames, flights) = play(19, key);
    assert_eq!(flights, [19, 21]);
    assert!(r.advance && !r.game_over, "{r:?}");
    let after19 = after_flight(&frames, 1);
    let launch21 = frames.iter().position(|f| f.launches == 2).unwrap();
    let between = cues_from(&frames[..launch21], after19);
    assert!(between.contains(&Some(5)), "{between:?}");
    assert!(
        !between.contains(&Some(3)),
        "the M3 hangar card is skipped: {between:?}"
    );
    // After flight 21 the story goes on (mood3, cue 13).
    let after21 = after_flight(&frames, 2);
    assert_eq!(
        cues_from(&frames, after21).iter().flatten().next(),
        Some(&13)
    );
}

#[test]
fn block_19_esc_after_flight_21_ends_the_block() {
    // screens §3.12: after flight 21 a set skip flag ends the block (the mission counts).
    let _data = require_data!();
    let key = |f: &Frame, last: Option<u8>| {
        let esc = last == Some(21) && !f.in_flight;
        Some(if esc { Key::ESCAPE } else { Key::SPACE })
    };
    let (r, frames, flights) = play(19, key);
    assert_eq!(flights, [19, 21]);
    assert!(r.advance && !r.game_over, "{r:?}");
    let after21 = after_flight(&frames, 2);
    let cues = cues_from(&frames, after21);
    assert!(
        !cues.contains(&Some(5)),
        "the block ended before M5: {cues:?}"
    );
}

#[test]
fn flight_21_abort_ends_the_block_without_advancing() {
    // episode-program-flow rev 5 §4.4 (U-081): only the abort flag is tested after flight 21;
    // abort → return with the mission not advanced; no retry, no low, no debrief.
    let _data = require_data!();
    let key = |f: &Frame, last: Option<u8>| {
        let abort = last == Some(21) && f.in_flight;
        Some(if abort {
            Key::letter('a').unwrap()
        } else {
            Key::SPACE
        })
    };
    let (r, frames, flights) = play(19, key);
    assert_eq!(flights, [19, 21]);
    assert!(!r.advance && !r.game_over, "{r:?}");
    let after21 = after_flight(&frames, 2);
    let tail = &frames[after21..];
    assert!(
        tail.iter()
            .all(|f| !f.retry && !f.debrief && f.cue != Some(8))
    );
}

#[test]
fn flight_21_death_is_ignored() {
    // Flight 21 always ends through the end sequence (died = 1); the block ignores it: no retry
    // prompt, no low, no debrief, no ending; the post-flight story runs and the mission counts.
    let _data = require_data!();
    let key = |f: &Frame, last: Option<u8>| {
        let died = last == Some(21) && f.in_flight;
        Some(if died {
            Key::letter('d').unwrap()
        } else {
            Key::SPACE
        })
    };
    let (r, frames, flights) = play(19, key);
    assert_eq!(flights, [19, 21]);
    assert!(r.advance && !r.game_over, "{r:?}");
    // U-420 (episode-program-flow rev 6): the block hands back flight 21's kills (0 here),
    // not flight 19's (3).
    assert_eq!(r.mission_kills, 0, "{r:?}");
    let after21 = after_flight(&frames, 2);
    let tail = &frames[after21..];
    assert!(tail.iter().all(|f| !f.retry && !f.debrief));
    let cues = cues_from(&frames, after21);
    assert!(
        !cues.contains(&Some(8)) && !cues.contains(&Some(12)),
        "{cues:?}"
    );
    assert_eq!(cues.iter().flatten().next(), Some(&13), "{cues:?}");
}
