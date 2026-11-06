//! The Episode 3 front end against the original data and the oracle captures C-0036 and C-0041
//! (`docs/spec/flow/e3-program-flow.md` rev 1 §4, §6, §8), and the block sequence of
//! `td_flow::campaign_e3`. Data-gated (TD2192_DATA; the frame comparisons also
//! TD2192_ORACLE). No captured pixels are stored.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use td_assets::{DataDir, Episode, ExeResources};
use td_audio::AudioCommand;
use td_flow::campaign_e2::{BlockResult, Cap};
use td_flow::campaign_e3::{self, CampaignE3, E3_BLOCKS, Step};
use td_flow::program::Program;
use td_flow::program_e2::BlockStart;
use td_flow::scenes_e3::{self, Bg};
use td_flow::settings::Settings;
use td_flow::story::{StoryAssets, StoryScreen};
use td_flow::{
    Episode3, FlightLauncher, FlightResult, FlightScreen, FlowContext, Screen, Transition,
};
use td_formats::palette::{Palette, Rgb6};
use td_input::{Key, TickInput};
use td_render::Framebuffer;
use td_test_support::{require_data, require_oracle};

fn load() -> Option<(DataDir, ExeResources)> {
    let data = td_test_support::data_dir()?;
    let dir = DataDir::open(&data).unwrap();
    let res = ExeResources::load(&dir).unwrap();
    Some((dir, res))
}

fn oracle_frame(oracle: &Path, capture: &str, name: &str) -> (Palette, Vec<u8>) {
    let raw = fs::read(oracle.join(capture).join("fb").join(name)).unwrap();
    (Palette::parse_prefix(&raw).unwrap(), raw[768..].to_vec())
}

/// Every pixel, and the DAC entries the captured frame uses.
fn compare(ours: &Framebuffer, oracle: &Path, capture: &str, name: &str) {
    let (dac, frame) = oracle_frame(oracle, capture, name);
    let diffs: Vec<String> = (0..64_000)
        .filter(|&i| ours.pixels[i] != frame[i])
        .take(12)
        .map(|i| {
            format!(
                "({}, {}): {} vs {}",
                i % 320,
                i / 320,
                ours.pixels[i],
                frame[i]
            )
        })
        .collect();
    assert!(
        diffs.is_empty(),
        "{capture} {name}: pixels differ: {diffs:?}"
    );
    for e in frame.iter().copied().collect::<BTreeSet<u8>>() {
        let e = usize::from(e);
        assert_eq!(
            ours.palette.colors[e], dac.colors[e],
            "{capture} {name}: DAC entry {e}"
        );
    }
}

fn faded_in(mut s: StoryScreen) -> StoryScreen {
    for _ in 0..40 {
        s.step(&TickInput::default());
    }
    s
}

/// A flight that ends at once with a scripted result.
#[derive(Debug)]
struct Instant(FlightResult);

impl Screen for Instant {
    fn tick(&mut self, _ctx: &mut FlowContext<'_>, _input: &TickInput) -> Transition {
        Transition::Pop
    }
    fn draw(&self, fb: &mut Framebuffer) {
        fb.clear(0);
    }
}

impl FlightScreen for Instant {
    fn result(&self) -> FlightResult {
        self.0
    }
}

type Shared<T> = Rc<RefCell<T>>;

/// Flies the scripted results in order (then plain successes with 2 kills), counting
/// `Random` calls.
#[derive(Debug, Default)]
struct Scripted {
    results: Shared<Vec<FlightResult>>,
    flights: Shared<Vec<u8>>,
    randoms: Rc<Cell<u32>>,
}

impl FlightLauncher for Scripted {
    fn launch(&mut self, _ctx: &mut FlowContext<'_>, flight: u8) -> Box<dyn FlightScreen> {
        self.flights.borrow_mut().push(flight);
        let mut results = self.results.borrow_mut();
        let r = if results.is_empty() {
            FlightResult {
                kills: 2,
                outcome: true,
                ..FlightResult::default()
            }
        } else {
            results.remove(0)
        };
        Box::new(Instant(r))
    }

    fn random(&mut self, _n: u16) -> u16 {
        self.randoms.set(self.randoms.get() + 1);
        0
    }
}

fn died() -> FlightResult {
    FlightResult {
        died: true,
        kills: 1,
        ..FlightResult::default()
    }
}

fn result(f: impl FnOnce(&mut FlightResult)) -> FlightResult {
    let mut r = FlightResult {
        kills: 4,
        outcome: true,
        ..FlightResult::default()
    };
    f(&mut r);
    r
}

struct Run {
    block: BlockResult,
    flights: Vec<u8>,
    randoms: u32,
    audio: Vec<AudioCommand>,
    /// Story steps in the order they were shown.
    shown: Vec<Step>,
    /// Debriefs shown.
    debriefs: u32,
}

/// Plays block `mission` (kills to date 10) with the scripted flight results; at a retry
/// prompt presses `retry(n)` for the n-th prompt (true = fly again), elsewhere Space every 20
/// frames.
fn play(mission: u8, results: Vec<FlightResult>, retry: impl Fn(u32) -> bool) -> Run {
    play_from(
        BlockStart {
            mission,
            kills_to_date: 10,
            ..BlockStart::default()
        },
        results,
        retry,
    )
}

fn play_from(start: BlockStart, results: Vec<FlightResult>, retry: impl Fn(u32) -> bool) -> Run {
    let (dir, res) = load().unwrap();
    let launcher = Scripted::default();
    let (flights, randoms) = (launcher.flights.clone(), launcher.randoms.clone());
    *launcher.results.borrow_mut() = results;
    let mut prog = Program::for_episode(&dir, &res, Box::new(launcher), Episode::E3).unwrap();
    let settings = Settings::default();
    let (mut audio, mut all, mut shown) = (Vec::new(), Vec::new(), Vec::new());
    let mut c = CampaignE3::new(start);
    let (mut prompts, mut debriefs, mut in_debrief) = (0, 0, false);
    let mut prev = None;
    for frame in 0..2_000_000u32 {
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        // A screen that appears (a repeated one counts again after another screen or a gap).
        let now = c.current_step();
        if let Some(s) = now
            && now != prev
            && s != Step::B
        {
            shown.push(s);
        }
        prev = now;
        if c.in_debrief() && !in_debrief {
            debriefs += 1;
        }
        in_debrief = c.in_debrief();
        let key = if frame % 20 != 0 {
            None
        } else if c.in_retry_prompt() {
            prompts += 1;
            Some(if retry(prompts) {
                Key::SPACE
            } else {
                Key::ESCAPE
            })
        } else {
            Some(Key::SPACE)
        };
        let input = TickInput {
            pressed: key.into_iter().collect(),
            ..TickInput::default()
        };
        let r = c.step(&mut prog, &mut ctx, &input);
        all.append(&mut audio);
        if let Some(block) = r {
            return Run {
                block,
                flights: flights.borrow().clone(),
                randoms: randoms.get(),
                audio: all,
                shown,
                debriefs,
            };
        }
    }
    panic!("block {} never ended", start.mission);
}

fn songs(audio: &[AudioCommand]) -> usize {
    audio
        .iter()
        .filter(|a| matches!(a, AudioCommand::PlayMusic { .. }))
        .count()
}

fn names(shown: &[Step]) -> Vec<String> {
    shown
        .iter()
        .map(|s| match s {
            Step::S(bg, _) => format!("{bg:?}"),
            Step::D(n) => format!("D{n}"),
            Step::T(n) => format!("T{n}"),
            Step::Tc(_) => "Tc".to_owned(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn every_caption_and_title_resolves() {
    let _data = require_data!();
    let (dir, res) = load().unwrap();
    for block in &E3_BLOCKS {
        for s in block.steps {
            let caps: Vec<Cap> = match *s {
                Step::T(n) => {
                    assert!(
                        res.text(&format!("e3.mission.{n:02}.title")).is_some(),
                        "title {n}"
                    );
                    Vec::new()
                }
                Step::S(_, caps) => caps.to_vec(),
                Step::Tc(c) => vec![c],
                Step::TEnd => {
                    assert!(res.text("e3.ending").is_some());
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for c in caps {
                let (id, k) = c.resource_in(Episode::E3);
                let list = res.texts(&id).unwrap_or_else(|| panic!("{id}"));
                assert!(k < list.len(), "block {}: {id}[{k}]", block.number);
            }
        }
    }
    let (id, k) = campaign_e3::BLANK.resource_in(Episode::E3);
    assert_eq!(res.texts(&id).unwrap()[k], " ", "`_` is a single space");
    for bg in [
        Bg::Cmd,
        Bg::Cnv,
        Bg::In2,
        Bg::Ion,
        Bg::Mar,
        Bg::Mel,
        Bg::Mn1,
        Bg::Mn2,
        Bg::Mr2,
        Bg::Mr3,
        Bg::Kol,
        Bg::Sel,
        Bg::Sh0,
        Bg::Vhg,
        Bg::Vid,
        Bg::Vlb,
        Bg::Lab,
        Bg::Aboutus,
        Bg::Order,
    ] {
        assert!(dir.resolve(bg.layout().file).is_some(), "{bg:?}");
    }
}

#[test]
fn every_block_plays_to_the_end() {
    let _data = require_data!();
    for b in &E3_BLOCKS {
        let m = b.number;
        let r = play(m, Vec::new(), |_| false);
        assert!(
            r.block.advance && !r.block.game_over,
            "block {m}: {:?}",
            r.block
        );
        assert_eq!(r.block.mission_kills, 2);
        let want: Vec<u8> = match m {
            6 => vec![6, 7],
            11 => vec![11, 21],
            m => vec![m],
        };
        assert_eq!(r.flights, want, "block {m}");
        assert_eq!(r.randoms, 0, "block {m}: no retry prompt");
        let debriefs = match m {
            6 => 2,
            19 => 0,
            _ => 1,
        };
        assert_eq!(r.debriefs, debriefs, "block {m}");
        assert_eq!(r.block.set_mission, (m == 6).then_some(7), "block {m}");
    }
}

#[test]
fn block_1_follows_the_spec() {
    // §4.4 block 1, §8 "without Esc".
    let _data = require_data!();
    let r = play(1, Vec::new(), |_| false);
    assert_eq!(
        names(&r.shown),
        [
            "Sh0", "D1", "Tc", "D2", "Sh0", "D3", "Sh0", "D4", "Mar", "D5", "T1", "Mn1", "D6",
            "Mn1", "D7", "Mn1", "D8", "Cmd", "D201"
        ]
    );
}

#[test]
fn block_6_flies_f6_then_f7() {
    // §4.4.1 and §6 vector 10: two debriefs headed "mission 6", mission := 7 for the menu.
    let _data = require_data!();
    let r = play(6, Vec::new(), |_| false);
    assert_eq!(r.flights, [6, 7]);
    assert!(r.shown.contains(&Step::D(41)) && r.shown.contains(&Step::D(42)));
    // Death in F7, accepted: F7 again, not F6.
    let again = play(6, vec![result(|_| {}), died()], |_| true);
    assert_eq!(again.flights, [6, 7, 7]);
    assert_eq!(again.block.set_mission, Some(7));
    // Aborted in F7: return, advance 0, no store of 7 (Next mission replays block 6).
    let abort = play(
        6,
        vec![result(|_| {}), result(|r| r.aborted = true)],
        |_| false,
    );
    assert!(!abort.block.advance && abort.block.set_mission.is_none());
}

#[test]
fn block_11_flies_f11_then_f21() {
    // §4.4.2, §6 vector 11.
    let _data = require_data!();
    let r = play(11, Vec::new(), |_| false);
    assert_eq!(r.flights, [11, 21]);
    assert_eq!(r.debriefs, 1, "only F21 has a debrief");
    // F21 died, accepted: the debrief first, then F11 (with M13, D64) and F21 again.
    let again = play(11, vec![result(|_| {}), died()], |_| true);
    assert_eq!(again.flights, [11, 21, 11, 21]);
    assert_eq!(again.debriefs, 2);
    assert_eq!(again.shown.iter().filter(|&&s| s == Step::D(64)).count(), 2);
    // F21 aborted: the debrief, then return without advance.
    let abort = play(
        11,
        vec![result(|_| {}), result(|r| r.aborted = true)],
        |_| false,
    );
    assert!(!abort.block.advance && abort.debriefs == 1);
    // F11 died, declined: ending A without a debrief.
    let dead = play(11, vec![died()], |_| false);
    assert!(dead.block.game_over && dead.debriefs == 0);
    assert!(dead.shown.contains(&Step::Dossier));
}

#[test]
fn replay_blocks_offer_the_block_again() {
    // §6 vector 7 (OB-027): outcome 0, Esc at the retry: replay, kills of the attempt count.
    let _data = require_data!();
    for m in [2, 3, 4, 5, 9, 12] {
        let r = play(m, vec![result(|r| r.outcome = false)], |_| false);
        assert_eq!(
            r.block,
            BlockResult {
                advance: true,
                mission_kills: 4,
                replay: true,
                ..BlockResult::default()
            },
            "block {m}"
        );
        assert_eq!(r.randoms, 1);
        let accepted = play(m, vec![result(|r| r.outcome = false)], |_| true);
        assert_eq!(accepted.flights, [m, m]);
        assert!(accepted.block.advance && !accepted.block.replay);
    }
    // Other blocks ignore outcome 0 (no rule), e.g. block 13.
    let r = play(13, vec![result(|r| r.outcome = false)], |_| false);
    assert!(r.block.advance && r.randoms == 0);
}

#[test]
fn block_8_decline_counts_as_a_success() {
    // §6 vector 9: mn2 card, D49, the prompt; Esc → D50, kolorvci, D51; advance.
    let _data = require_data!();
    let r = play(8, vec![result(|r| r.outcome = false)], |_| false);
    assert!(r.block.advance && !r.block.replay);
    let tail = names(&r.shown);
    let i = tail.iter().position(|s| s == "D49").unwrap();
    assert_eq!(&tail[i - 1..], ["Mn2", "D49", "Cmd", "D50", "Kol", "D51"]);
    let ok = play(8, Vec::new(), |_| false);
    assert!(!ok.shown.contains(&Step::D(49)));
    let again = play(8, vec![result(|r| r.outcome = false)], |_| true);
    assert_eq!(again.flights, [8, 8]);
}

#[test]
fn block_10_decline_keeps_the_mission() {
    // §6 vector 8: outcome 0 → mn2 card, D59, prompt; Esc → advance 0.
    let _data = require_data!();
    let r = play(10, vec![result(|r| r.outcome = false)], |_| false);
    assert!(!r.block.advance && !r.block.game_over);
    assert!(r.shown.contains(&Step::D(59)) && !r.shown.contains(&Step::D(60)));
    let ok = play(10, Vec::new(), |_| false);
    assert!(ok.shown.contains(&Step::D(60)) && !ok.shown.contains(&Step::D(59)));
    assert!(ok.block.advance);
}

#[test]
fn death_declined_is_ending_a() {
    let _data = require_data!();
    let r = play(4, vec![died()], |_| false);
    assert!(r.block.game_over && !r.block.advance, "{:?}", r.block);
    assert_eq!(r.randoms, 1, "one Random(10) per prompt");
    assert!(r.shown.contains(&Step::Dossier));
    assert_eq!(r.debriefs, 0);
    let loops: Vec<bool> = r
        .audio
        .iter()
        .filter_map(|a| match a {
            AudioCommand::SetMusicLoop(l) => Some(*l),
            _ => None,
        })
        .collect();
    assert_eq!(loops, [false, true]);
}

#[test]
fn block_19_ends_with_the_end_card_and_no_debrief() {
    // §4.5, §6 vector 12.
    let _data = require_data!();
    let r = play(19, Vec::new(), |_| false);
    assert!(r.block.advance && r.debriefs == 0);
    assert_eq!(r.shown.last(), Some(&Step::TEnd));
}

#[test]
fn missions_without_a_block_add_the_stale_kills() {
    // OB-051: missions 7 and 20 run nothing and hand back the stale mission kills.
    let _data = require_data!();
    for m in [7, 20, 21] {
        let r = play_from(
            BlockStart {
                mission: m,
                kills_to_date: 0,
                skip: true,
                mission_kills: 3,
            },
            Vec::new(),
            |_| false,
        );
        assert!(r.flights.is_empty() && r.shown.is_empty());
        assert!(r.block.advance && r.block.mission_kills == 3 && r.block.skip);
    }
}

#[test]
fn block_1_skip_shortcut() {
    // OB-050: with the skip flag set, block 1 goes straight to flight 1, no card, no M6.
    let _data = require_data!();
    let r = play_from(
        BlockStart {
            mission: 1,
            skip: true,
            ..BlockStart::default()
        },
        Vec::new(),
        |_| false,
    );
    assert_eq!(r.flights, [1]);
    assert_eq!(names(&r.shown)[0], "Mn1", "the post-flight story only");
    // Blocks 2..19 do not test it before their first screen.
    let r = play_from(
        BlockStart {
            mission: 2,
            skip: true,
            ..BlockStart::default()
        },
        Vec::new(),
        |_| false,
    );
    assert_eq!(names(&r.shown)[0], "Cmd");
}

#[test]
fn esc_at_the_first_card_goes_straight_to_the_flight() {
    // §8: Esc during shuttle0's wait (after its fade-in) skips D1..D7; an Esc during the
    // fade-in is flushed when the wait starts.
    let _data = require_data!();
    for (frame, flies) in [(5, false), (60, true)] {
        let (dir, res) = load().unwrap();
        let launcher = Scripted::default();
        let flights = launcher.flights.clone();
        let mut prog = Program::for_episode(&dir, &res, Box::new(launcher), Episode::E3).unwrap();
        let settings = Settings::default();
        let mut audio = Vec::new();
        let mut c = CampaignE3::new(BlockStart {
            mission: 1,
            ..BlockStart::default()
        });
        for f in 0..200 {
            let mut ctx = FlowContext {
                data: &dir,
                resources: &res,
                settings: &settings,
                audio: &mut audio,
            };
            let pressed = if f == frame {
                vec![Key::ESCAPE]
            } else {
                Vec::new()
            };
            c.step(
                &mut prog,
                &mut ctx,
                &TickInput {
                    pressed,
                    ..TickInput::default()
                },
            );
            if !flights.borrow().is_empty() {
                break;
            }
        }
        assert_eq!(!flights.borrow().is_empty(), flies, "Esc at frame {frame}");
        if flies {
            // mood, then the flight song (td01).
            assert_eq!(songs(&audio), 2);
        }
    }
}

/// Runs the program for `frames` frames with `keys(frame)`, stopping before a frame in which
/// `stop` holds.
fn run_until(
    ep: &mut Episode3,
    frames: u32,
    keys: impl Fn(u32) -> Vec<Key>,
    stop: impl Fn(&Episode3) -> bool,
) -> Vec<AudioCommand> {
    let (dir, res) = load().unwrap();
    let settings = Settings::default();
    let mut audio = Vec::new();
    for f in 0..frames {
        if f > 0 && stop(ep) {
            break;
        }
        let mut ctx = FlowContext {
            data: &dir,
            resources: &res,
            settings: &settings,
            audio: &mut audio,
        };
        let input = TickInput {
            pressed: keys(f),
            ..TickInput::default()
        };
        if matches!(ep.tick(&mut ctx, &input), Transition::Quit) {
            break;
        }
    }
    audio
}

fn run(ep: &mut Episode3, frames: u32, keys: impl Fn(u32) -> Vec<Key>) -> Vec<AudioCommand> {
    run_until(ep, frames, keys, |_| false)
}

fn program(launcher: Scripted) -> Episode3 {
    let (dir, res) = load().unwrap();
    Episode3::new(&dir, &res, Box::new(launcher)).unwrap()
}

const DIM: Rgb6 = Rgb6 { r: 16, g: 0, b: 0 };
const RED: Rgb6 = Rgb6 { r: 63, g: 0, b: 0 };

#[test]
fn start_up_to_the_menu_and_a_completed_block() {
    // §6 vectors 5 and 6: menu.CMF once; keys typed after the choice do not reach the menu.
    let _data = require_data!();
    let mut ep = program(Scripted::default());
    let two = Key::digit(2).unwrap();
    let audio = run(&mut ep, 100, |f| match f {
        50 => vec![two, Key::DOWN],
        _ => Vec::new(),
    });
    assert_eq!(songs(&audio), 1, "menu.CMF started once");
    assert_eq!(ep.state(), (0, 0, 0), "the Down typed after 2 was flushed");
    let c = ep.item_colours();
    assert_eq!(c[0], RED);
    assert!(c[1..].iter().all(|&e| e == DIM));
    // New game (Enter), then Space through block 1, until the menu is back.
    let keys = |f| match f {
        0 => vec![Key::ENTER],
        f if f % 20 == 0 => vec![Key::SPACE],
        _ => Vec::new(),
    };
    run_until(&mut ep, 400_000, keys, |ep| ep.state().0 == 2);
    assert_eq!(ep.state(), (2, 2, 3));
    assert_eq!(ep.item_colours()[3], RED, "OB-041: red, not yellow");
}

#[test]
fn block_6_and_episode_end_bookkeeping() {
    // §4.6: block 6 → mission 8 (7 + 1), kills += F7's; block 19 → mission 0, cursor 7.
    let _data = require_data!();
    let space = |f| match f {
        0 => vec![Key::ENTER],
        f if f % 20 == 0 => vec![Key::SPACE],
        _ => Vec::new(),
    };
    let mut ep = program(Scripted::default());
    run(&mut ep, 3, |_| Vec::new());
    ep.restored(6, 10);
    run_until(&mut ep, 600_000, space, |ep| ep.state().0 != 6);
    assert_eq!(ep.state(), (8, 12, 3));
    let mut ep = program(Scripted::default());
    run(&mut ep, 3, |_| Vec::new());
    ep.restored(19, 50);
    run_until(&mut ep, 600_000, space, |ep| ep.state().0 != 19);
    assert_eq!(ep.state(), (0, 52, 7));
    // OB-051: a restored mission 7 runs nothing; mission 8, kills += 0 (stale after start-up).
    let mut ep = program(Scripted::default());
    run(&mut ep, 3, |_| Vec::new());
    ep.restored(7, 5);
    run_until(&mut ep, 1_000, space, |ep| ep.state().0 != 7);
    assert_eq!(ep.state(), (8, 5, 3));
}

#[test]
fn block_10_decline_and_game_over_bookkeeping() {
    let _data = require_data!();
    let esc = |f| match f {
        0 => vec![Key::ENTER],
        f if f % 20 == 0 => vec![Key::ESCAPE],
        _ => Vec::new(),
    };
    // §6 vector 8: block 10, outcome 0, Esc everywhere (the title card skips to the flight,
    // Esc at the retry declines): mission 10, kills unchanged.
    let launcher = Scripted::default();
    let flights = launcher.flights.clone();
    launcher
        .results
        .borrow_mut()
        .push(result(|r| r.outcome = false));
    let mut ep = program(launcher);
    run(&mut ep, 3, |_| Vec::new());
    ep.restored(10, 7);
    run(&mut ep, 20_000, esc);
    assert_eq!(*flights.borrow(), [10]);
    assert_eq!((ep.state().0, ep.state().1), (10, 7));
    // Item 3 path, game over: mission 0, cursor 0.
    let launcher = Scripted::default();
    launcher.results.borrow_mut().push(died());
    let mut ep = program(launcher);
    run(&mut ep, 3, |_| Vec::new());
    ep.restored(9, 0);
    run_until(&mut ep, 400_000, esc, |ep| ep.state().0 != 9);
    assert_eq!(ep.state(), (0, 0, 0));
}

#[test]
fn esc_on_the_about_screen_skips_block_1() {
    // §6 vector 13 (OB-050): item 6, Esc on the about screen, then item 0: flight 1 at once.
    let _data = require_data!();
    let launcher = Scripted::default();
    let flights = launcher.flights.clone();
    let mut ep = program(launcher);
    let two = Key::digit(2).unwrap();
    run(
        &mut ep,
        100,
        |f| if f == 10 { vec![two] } else { Vec::new() },
    );
    // Down four times: 0 → 2 → 4 → 5 → 6 (items 1 and 3 are skipped with no game).
    let keys = |f| match f {
        0..=3 => vec![Key::DOWN],
        4 => vec![Key::ENTER],
        100 => vec![Key::ESCAPE],
        _ => Vec::new(),
    };
    run_until(&mut ep, 400, keys, |ep| ep.skip_flag());
    assert!(ep.skip_flag(), "Esc on the about screen sets the flag");
    run(&mut ep, 200, |_| Vec::new());
    assert!(ep.skip_flag(), "the menu does not clear it");
    // Up from 6: 5, 4, 2, 0 (one key per frame; the menu discards the rest).
    run(&mut ep, 4, |_| vec![Key::UP]);
    assert_eq!(ep.state().2, 0, "cursor back on item 0");
    run_until(
        &mut ep,
        10,
        |f| if f == 0 { vec![Key::ENTER] } else { Vec::new() },
        |_| !flights.borrow().is_empty(),
    );
    assert_eq!(*flights.borrow(), [1], "no card before flight 1");
}

#[test]
fn script_choice_matches_c0036() {
    let _data = require_data!();
    let oracle = require_oracle!();
    let mut ep = program(Scripted::default());
    run(&mut ep, 40, |_| Vec::new());
    let mut fb = Framebuffer::new();
    ep.draw(&mut fb);
    compare(
        &fb,
        &oracle,
        "C-0036",
        "f00000146_320x200_script-choice.vga",
    );
}

#[test]
fn main_menu_matches_c0036() {
    // OB-041 in E3: DAC 224 = (63, 0, 0), 225..233 = (16, 0, 0) including 227.
    let _data = require_data!();
    let oracle = require_oracle!();
    let mut ep = program(Scripted::default());
    let two = Key::digit(2).unwrap();
    run(
        &mut ep,
        100,
        |f| if f == 10 { vec![two] } else { Vec::new() },
    );
    let mut fb = Framebuffer::new();
    ep.draw(&mut fb);
    let name = "f00000373_320x200_main-menu.vga";
    compare(&fb, &oracle, "C-0036", name);
    let (dac, _) = oracle_frame(&oracle, "C-0036", name);
    for e in 0..256 {
        assert_eq!(fb.palette.colors[e], dac.colors[e], "entry {e}");
    }
}

fn caption_texts(res: &ExeResources, caps: [Cap; 3]) -> [String; 3] {
    caps.map(|c| {
        let (id, k) = c.resource_in(Episode::E3);
        res.texts(&id).unwrap()[k].clone()
    })
}

#[test]
fn shuttle0_card_matches_c0036() {
    // Block 1's first card: shuttle0[p.0/p.1/_] (C-0036 f570).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let assets = StoryAssets::load(&dir).unwrap();
    let t = caption_texts(&res, [Cap::P(0), Cap::P(1), campaign_e3::BLANK]);
    let s = scenes_e3::scene_card(&dir, &assets, Bg::Sh0, [&t[0], &t[1], &t[2]]).unwrap();
    let s = faded_in(s);
    compare(
        s.screen(),
        &oracle,
        "C-0036",
        "f00000570_320x200_shuttle0.vga",
    );
}

#[test]
fn maraud3_card_matches_c0041() {
    // Block 8's first card: maraud3[7.2/_/_] (C-0041 f946).
    let _data = require_data!();
    let oracle = require_oracle!();
    let (dir, res) = load().unwrap();
    let assets = StoryAssets::load(&dir).unwrap();
    let blank = campaign_e3::BLANK;
    let t = caption_texts(&res, [Cap::C(7, 2), blank, blank]);
    let s = scenes_e3::scene_card(&dir, &assets, Bg::Mr3, [&t[0], &t[1], &t[2]]).unwrap();
    let s = faded_in(s);
    compare(
        s.screen(),
        &oracle,
        "C-0041",
        "f00000946_320x200_maraud3.vga",
    );
}

#[test]
fn restore_screen_and_menu_match_c0041() {
    // §8 (C-0041): the shipped td3.sav with slot 1 := 08 00 00; Down, Enter at the menu; the
    // restore screen (f576); Enter on slot 1; the menu with mission 8 and the cursor on item 3.
    let data = require_data!();
    let oracle = require_oracle!();
    let saves = std::env::temp_dir().join(format!("td2192-e3-restore-{}", std::process::id()));
    fs::create_dir_all(&saves).unwrap();
    let mut sav = fs::read(data.join("td3.sav")).unwrap();
    sav[..3].copy_from_slice(&[8, 0, 0]);
    fs::write(saves.join("td3.sav"), sav).unwrap();
    fs::copy(data.join("td3.stt"), saves.join("td3.stt")).unwrap();
    let mut ep = program(Scripted::default()).with_save_dir(saves.clone());
    let two = Key::digit(2).unwrap();
    run(
        &mut ep,
        100,
        |f| if f == 10 { vec![two] } else { Vec::new() },
    );
    run(&mut ep, 60, |f| match f {
        0 => vec![Key::DOWN],
        1 => vec![Key::ENTER],
        _ => Vec::new(),
    });
    let mut fb = Framebuffer::new();
    ep.draw(&mut fb);
    compare(&fb, &oracle, "C-0041", "f00000576_320x200_restore.vga");
    run(&mut ep, 60, |f| {
        if f == 0 { vec![Key::ENTER] } else { Vec::new() }
    });
    assert_eq!(ep.state(), (8, 0, 3));
    let c = ep.item_colours();
    assert_eq!((c[2], c[3]), (DIM, RED));
    fs::remove_dir_all(&saves).unwrap();
}
